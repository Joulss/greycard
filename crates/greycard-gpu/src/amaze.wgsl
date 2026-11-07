// AMaZE (Aliasing Minimization and Zipper Elimination) on the GPU, as
// greycard-core's port has it (`greycard_core::develop::amaze`, the
// reference here). That port is of RawTherapee's
// `rtengine/amaze_demosaic_RT.cc`, GPL-3.0-or-later, copyright (c)
// 2008-2010 Emil Martinec <ejmartin@uchicago.edu>, optimized for speed
// by Ingo Weyrich, incorporating ideas of Luis Sanz Rodríguez and Paul
// Lee. The algorithm, its constants, and its passes are theirs; every
// expression here takes the reference's operations in the reference's
// order.
//
// The reference works in tiles of 160 with a reflected margin of 16 and
// its output depends on the tile edges (cells a pass never writes are
// read as zero, and the Nyquist box is a tile's own), so we keep its
// tiles: every plane is a 160x160 block per tile, a batch of tiles at a
// time, and each pass is a dispatch over the batch. Three passes are
// recursions in the reference's raster order and run as such: the
// third (a thread walks a row for the horizontal difference and a
// column for the vertical), the sixth and the ninth (a workgroup a
// tile, a thread a column, the rows in order behind a barrier).

struct Params {
    // The mosaic's size.
    size: vec2<u32>,
    // Tiles across the picture.
    tiles_across: u32,
    // The batch's first tile, counted across then down, and how many.
    first_tile: u32,
    count: u32,
    // The picture row the output band starts at.
    band_y0: u32,
    // Outputs are clamped to 0..ceiling.
    ceiling: f32,
    pad: u32,
    // The color at each parity (row & 1, col & 1), in the order
    // (0,0) (0,1) (1,0) (1,1): 0 red, 1 green, 2 blue.
    colors: vec4<u32>,
}

@group(0) @binding(0) var<uniform> P: Params;
@group(0) @binding(1) var mosaic: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> s0: array<f32>;
@group(0) @binding(3) var<storage, read_write> s1: array<f32>;
@group(0) @binding(4) var<storage, read_write> s2: array<f32>;
@group(0) @binding(5) var<storage, read_write> s3: array<f32>;
// Per tile: the Nyquist box's first row, last row, first and last
// column.
@group(0) @binding(6) var<storage, read_write> boxes: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> band: array<f32>;

const TS: i32 = 160;
const MARGIN: i32 = 16;
// The step between tiles, their interior's side.
const STEP: i32 = TS - 2 * MARGIN;
const CELLS: u32 = u32(TS * TS);
// The planes a tile, seven to a binding (the host's checked against
// these by a unit test).
const PLANES: u32 = 27u;
const PER_BINDING: u32 = 7u;

const V1: i32 = 160;
const V2: i32 = 320;
const V3: i32 = 480;
// p: towards north-east; m: towards south-east.
const P1: i32 = -159;
const P2: i32 = -318;
const P3: i32 = -477;
const M1: i32 = 161;
const M2: i32 = 322;
const M3: i32 = 483;

const EPS: f32 = 1e-5;
const EPS_SQ: f32 = 1e-10;
const AR_THRESH: f32 = 0.75;
const CLIP_PT: f32 = 1.0;
const CLIP_PT8: f32 = 0.8;
const GAUSS_ODD: array<f32, 4> = array<f32, 4>(0.14659728, 0.10359271, 0.07320361, 0.03655435);
const GAUSS_GRAD: array<f32, 6> = array<f32, 6>(
    0.5 * 0.07384412,
    0.5 * 0.06207512,
    0.5 * 0.05218182,
    0.5 * 0.03687419,
    0.5 * 0.03099732,
    0.5 * 0.01841319,
);
const GAUSS_EVEN: array<f32, 2> = array<f32, 2>(0.13719494, 0.05640253);
const G_QUINC: array<f32, 4> = array<f32, 4>(0.169917, 0.108947, 0.069855, 0.0287182);

// The planes, seven to a binding.
const CFA: u32 = 0u;
const GREEN: u32 = 1u;
const DELHVSQSUM: u32 = 2u;
const DIRWTS0: u32 = 3u;
const DIRWTS1: u32 = 4u;
const VCD: u32 = 5u;
const HCD: u32 = 6u;
const VCDALT: u32 = 7u;
const HCDALT: u32 = 8u;
const CDDIFFSQ: u32 = 9u;
const HVWT: u32 = 10u;
const DGRB0: u32 = 11u;
const DGRB1: u32 = 12u;
const DGINTV: u32 = 13u;
const DGINTH: u32 = 14u;
const DGRB2H: u32 = 15u;
const DGRB2V: u32 = 16u;
const DELP: u32 = 17u;
const DELM: u32 = 18u;
const DGRBSQ1P: u32 = 19u;
const DGRBSQ1M: u32 = 20u;
const RBM: u32 = 21u;
const RBP: u32 = 22u;
const PMWT: u32 = 23u;
const RBINT: u32 = 24u;
const NYQUIST: u32 = 25u;
const NYQUIST2: u32 = 26u;

// The tile in hand: its index in the batch, its origin in the picture
// (margin included), and its extent (shorter at the far edges).
var<private> tile: u32;
var<private> top: i32;
var<private> left: i32;
var<private> rr1: i32;
var<private> cc1: i32;
// The cell in hand, in tile coordinates.
var<private> rr: i32;
var<private> cc: i32;
var<private> i: i32;

// (top, left, rr1, cc1) of a tile of the batch.
fn geometry(t: u32) -> vec4<i32> {
    let g = P.first_tile + t;
    let ty = i32(g / P.tiles_across);
    let tx = i32(g % P.tiles_across);
    let t0 = -MARGIN + STEP * ty;
    let l0 = -MARGIN + STEP * tx;
    let r1 = min(t0 + TS, i32(P.size.y) + MARGIN) - t0;
    let c1 = min(l0 + TS, i32(P.size.x) + MARGIN) - l0;
    return vec4<i32>(t0, l0, r1, c1);
}

fn set_tile(t: u32) {
    tile = t;
    let g = geometry(t);
    top = g.x;
    left = g.y;
    rr1 = g.z;
    cc1 = g.w;
}

fn set_cell(r: i32, c: i32) {
    rr = r;
    cc = c;
    i = r * TS + c;
}

// A thread a cell: x the column, y the row, z the tile.
fn enter(id: vec3<u32>) -> bool {
    set_tile(id.z);
    set_cell(i32(id.y), i32(id.x));
    return id.x < u32(TS) && id.y < u32(TS);
}

fn inside(m: i32) -> bool {
    return rr >= m && rr < rr1 - m && cc >= m && cc < cc1 - m;
}

fn color_at(r: i32, c: i32) -> u32 {
    return P.colors[u32((r & 1) * 2 + (c & 1))];
}

fn is_green() -> bool {
    return color_at(rr, cc) == 1u;
}

fn slot(p: u32, at: i32) -> u32 {
    return ((p % PER_BINDING) * P.count + tile) * CELLS + u32(at);
}

fn ld(p: u32, at: i32) -> f32 {
    let o = slot(p, at);
    switch p / PER_BINDING {
        case 0u: {
            return s0[o];
        }
        case 1u: {
            return s1[o];
        }
        case 2u: {
            return s2[o];
        }
        default: {
            return s3[o];
        }
    }
}

fn st(p: u32, at: i32, v: f32) {
    let o = slot(p, at);
    switch p / PER_BINDING {
        case 0u: {
            s0[o] = v;
        }
        case 1u: {
            s1[o] = v;
        }
        case 2u: {
            s2[o] = v;
        }
        default: {
            s3[o] = v;
        }
    }
}

// A plane at an offset from the cell in hand.
fn b(p: u32, o: i32) -> f32 {
    return ld(p, i + o);
}

fn s(o: i32) -> f32 {
    return ld(CFA, i + o);
}

fn sq(x: f32) -> f32 {
    return x * x;
}

fn median(x: f32, y: f32, z: f32) -> f32 {
    return max(min(max(x, y), z), min(x, y));
}

fn reflect(at: i32, n: i32) -> i32 {
    var r = at;
    if at < 0 {
        r = -at;
    } else if at >= n {
        r = 2 * n - 2 - at;
    }
    return clamp(r, 0, n - 1);
}

// The Nyquist box of the tile in hand, rows then columns, half-open;
// empty when the reference skips the Nyquist steps.
fn nyquist_box() -> vec4<i32> {
    let base = tile * 4u;
    let r0 = atomicLoad(&boxes[base]);
    let r1 = atomicLoad(&boxes[base + 1u]);
    let c0 = atomicLoad(&boxes[base + 2u]);
    let c1 = atomicLoad(&boxes[base + 3u]);
    if r0 == 0xffffffffu || r0 == r1 || c0 == c1 {
        return vec4<i32>(0, 0, 0, 0);
    }
    let y0 = i32(r0);
    let y1 = i32(r1);
    let x0 = i32(c0);
    let x1 = i32(c1);
    return vec4<i32>(max(y0, 8), min(y1 + 1, rr1 - 8), max(x0 - (x0 & 1), 8), min(x1 + 1, cc1 - 8));
}

fn in_box(box: vec4<i32>) -> bool {
    return rr >= box.x && rr < box.y && cc >= box.z && cc < box.w;
}

// The tile from the picture with reflected margins, every other plane
// zeroed as the reference's scratch is for each tile, and the Nyquist
// box emptied. Only the batch's tiles are touched.
@compute @workgroup_size(16, 16)
fn init(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) {
        return;
    }
    if rr == 0 && cc == 0 {
        let base = tile * 4u;
        atomicStore(&boxes[base], 0xffffffffu);
        atomicStore(&boxes[base + 1u], 0u);
        atomicStore(&boxes[base + 2u], 0xffffffffu);
        atomicStore(&boxes[base + 3u], 0u);
    }
    for (var p = 0u; p < PLANES; p++) {
        st(p, i, 0.0);
    }
    if rr < rr1 && cc < cc1 {
        let y = reflect(top + rr, i32(P.size.y));
        let x = reflect(left + cc, i32(P.size.x));
        let v = textureLoad(mosaic, vec2<i32>(x, y), 0).x;
        st(CFA, i, v);
        st(GREEN, i, v);
    }
}

// Pass 1: horizontal and vertical gradients.
@compute @workgroup_size(16, 16)
fn gradients(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(2) {
        return;
    }
    let delh = abs(s(1) - s(-1));
    let delv = abs(s(V1) - s(-V1));
    st(DIRWTS0, i, EPS + abs(s(V2) - s(0)) + abs(s(0) - s(-V2)) + delv);
    st(DIRWTS1, i, EPS + abs(s(2) - s(0)) + abs(s(0) - s(-2)) + delh);
    st(DELHVSQSUM, i, delh * delh + delv * delv);
}

fn ar(cr: f32, ha: f32) -> f32 {
    if abs(1.0 - cr) < AR_THRESH {
        return s(0) * cr;
    }
    return ha;
}

// Pass 2: vertical and horizontal color differences.
@compute @workgroup_size(16, 16)
fn differences(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(4) {
        return;
    }
    let cru = s(-V1) * (b(DIRWTS0, -V2) + b(DIRWTS0, 0))
        / (b(DIRWTS0, -V2) * (EPS + s(0)) + b(DIRWTS0, 0) * (EPS + s(-V2)));
    let crd = s(V1) * (b(DIRWTS0, V2) + b(DIRWTS0, 0))
        / (b(DIRWTS0, V2) * (EPS + s(0)) + b(DIRWTS0, 0) * (EPS + s(V2)));
    let crl = s(-1) * (b(DIRWTS1, -2) + b(DIRWTS1, 0))
        / (b(DIRWTS1, -2) * (EPS + s(0)) + b(DIRWTS1, 0) * (EPS + s(-2)));
    let crr = s(1) * (b(DIRWTS1, 2) + b(DIRWTS1, 0))
        / (b(DIRWTS1, 2) * (EPS + s(0)) + b(DIRWTS1, 0) * (EPS + s(2)));

    let guha = s(-V1) + 0.5 * (s(0) - s(-V2));
    let gdha = s(V1) + 0.5 * (s(0) - s(V2));
    let glha = s(-1) + 0.5 * (s(0) - s(-2));
    let grha = s(1) + 0.5 * (s(0) - s(2));

    var guar = ar(cru, guha);
    var gdar = ar(crd, gdha);
    var glar = ar(crl, glha);
    var grar = ar(crr, grha);

    let hwt = b(DIRWTS1, -1) / (b(DIRWTS1, -1) + b(DIRWTS1, 1));
    let vwt = b(DIRWTS0, -V1) / (b(DIRWTS0, V1) + b(DIRWTS0, -V1));

    let gintvha = vwt * gdha + (1.0 - vwt) * guha;
    let ginthha = hwt * grha + (1.0 - hwt) * glha;

    var vcd: f32;
    var hcd: f32;
    var vcdalt: f32;
    var hcdalt: f32;
    if is_green() {
        vcd = s(0) - (vwt * gdar + (1.0 - vwt) * guar);
        hcd = s(0) - (hwt * grar + (1.0 - hwt) * glar);
        vcdalt = s(0) - gintvha;
        hcdalt = s(0) - ginthha;
    } else {
        vcd = (vwt * gdar + (1.0 - vwt) * guar) - s(0);
        hcd = (hwt * grar + (1.0 - hwt) * glar) - s(0);
        vcdalt = gintvha - s(0);
        hcdalt = ginthha - s(0);
    }
    if s(0) > CLIP_PT8 || gintvha > CLIP_PT8 || ginthha > CLIP_PT8 {
        guar = guha;
        gdar = gdha;
        glar = glha;
        grar = grha;
        vcd = vcdalt;
        hcd = hcdalt;
    }
    st(VCD, i, vcd);
    st(HCD, i, hcd);
    st(VCDALT, i, vcdalt);
    st(HCDALT, i, hcdalt);
    st(DGINTV, i, min(sq(guha - gdha), sq(guar - gdar)));
    st(DGINTH, i, min(sq(glha - grha), sq(glar - grar)));
}

fn var3(x: f32, y: f32, z: f32) -> f32 {
    return 3.0 * (sq(x) + sq(y) + sq(z)) - sq(x + y + z);
}

// The bound of pass 3 for one axis: `cd` the chosen difference, `n`
// and `f` the cfa at the two neighbors along the axis.
fn bound(cd_in: f32, green: bool, n: f32, f: f32) -> f32 {
    var cd = cd_in;
    let s0v = s(0);
    if green {
        let gint = -cd + s0v;
        if cd > 0.0 {
            if 3.0 * cd > gint + s0v {
                cd = -median(gint, n, f) + s0v;
            } else {
                let wt = 1.0 - 3.0 * cd / (EPS + gint + s0v);
                cd = wt * cd + (1.0 - wt) * (-median(gint, n, f) + s0v);
            }
        }
        if gint > CLIP_PT {
            cd = -median(gint, n, f) + s0v;
        }
    } else {
        let gint = cd + s0v;
        if cd < 0.0 {
            if 3.0 * cd < -(gint + s0v) {
                cd = median(gint, n, f) - s0v;
            } else {
                let wt = 1.0 + 3.0 * cd / (EPS + gint + s0v);
                cd = wt * cd + (1.0 - wt) * (median(gint, n, f) - s0v);
            }
        }
        if gint > CLIP_PT {
            cd = median(gint, n, f) - s0v;
        }
    }
    return cd;
}

// Pass 3: choose the smoother pair, bound in saturated regions. In the
// reference's raster order the horizontal difference at a cell reads
// the one two to its left already rewritten, and the vertical the one
// two above: a thread walks a row (group x 0) or a column (group x 1)
// of a tile (group y).
@compute @workgroup_size(160)
fn choose(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    set_tile(wid.y);
    let k = i32(lid.x);
    if wid.x == 0u {
        if k < 4 || k >= rr1 - 4 {
            return;
        }
        for (var c = 4; c < cc1 - 4; c++) {
            set_cell(k, c);
            var hcd = ld(HCD, i);
            let hcdvar = var3(b(HCD, -2), hcd, b(HCD, 2));
            let hcdaltvar = var3(b(HCDALT, -2), b(HCDALT, 0), b(HCDALT, 2));
            if hcdaltvar < hcdvar {
                hcd = ld(HCDALT, i);
            }
            st(HCD, i, bound(hcd, is_green(), s(-1), s(1)));
        }
    } else {
        if k < 4 || k >= cc1 - 4 {
            return;
        }
        for (var r = 4; r < rr1 - 4; r++) {
            set_cell(r, k);
            var vcd = ld(VCD, i);
            let vcdvar = var3(b(VCD, -V2), vcd, b(VCD, V2));
            let vcdaltvar = var3(b(VCDALT, -V2), b(VCDALT, 0), b(VCDALT, V2));
            if vcdaltvar < vcdvar {
                vcd = ld(VCDALT, i);
            }
            st(VCD, i, bound(vcd, is_green(), s(-V1), s(V1)));
        }
    }
}

// The end of pass 3: the squared disagreement at red and blue sites.
@compute @workgroup_size(16, 16)
fn disagreement(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(4) || is_green() {
        return;
    }
    st(CDDIFFSQ, i, sq(ld(VCD, i) - ld(HCD, i)));
}

// Pass 4: vertical/horizontal weight at red and blue sites.
@compute @workgroup_size(16, 16)
fn weights(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(6) || is_green() {
        return;
    }
    let uave = b(VCD, 0) + b(VCD, -V1) + b(VCD, -V2) + b(VCD, -V3);
    let dave = b(VCD, 0) + b(VCD, V1) + b(VCD, V2) + b(VCD, V3);
    let lave = b(HCD, 0) + b(HCD, -1) + b(HCD, -2) + b(HCD, -3);
    let rave = b(HCD, 0) + b(HCD, 1) + b(HCD, 2) + b(HCD, 3);

    let varu = sq(b(VCD, 0) - uave) + sq(b(VCD, -V1) - uave) + sq(b(VCD, -V2) - uave)
        + sq(b(VCD, -V3) - uave);
    let vard = sq(b(VCD, 0) - dave) + sq(b(VCD, V1) - dave) + sq(b(VCD, V2) - dave)
        + sq(b(VCD, V3) - dave);
    let varl = sq(b(HCD, 0) - lave) + sq(b(HCD, -1) - lave) + sq(b(HCD, -2) - lave)
        + sq(b(HCD, -3) - lave);
    let varr = sq(b(HCD, 0) - rave) + sq(b(HCD, 1) - rave) + sq(b(HCD, 2) - rave)
        + sq(b(HCD, 3) - rave);

    let hwt = b(DIRWTS1, -1) / (b(DIRWTS1, -1) + b(DIRWTS1, 1));
    let vwt = b(DIRWTS0, -V1) / (b(DIRWTS0, V1) + b(DIRWTS0, -V1));

    let vcdvar = EPS_SQ + vwt * vard + (1.0 - vwt) * varu;
    let hcdvar = EPS_SQ + hwt * varr + (1.0 - hwt) * varl;

    let varu1 = b(DGINTV, 0) + b(DGINTV, -V1) + b(DGINTV, -V2);
    let vard1 = b(DGINTV, 0) + b(DGINTV, V1) + b(DGINTV, V2);
    let varl1 = b(DGINTH, 0) + b(DGINTH, -1) + b(DGINTH, -2);
    let varr1 = b(DGINTH, 0) + b(DGINTH, 1) + b(DGINTH, 2);
    let vcdvar1 = EPS_SQ + vwt * vard1 + (1.0 - vwt) * varu1;
    let hcdvar1 = EPS_SQ + hwt * varr1 + (1.0 - hwt) * varl1;

    let varwt = hcdvar / (vcdvar + hcdvar);
    let diffwt = hcdvar1 / (vcdvar1 + hcdvar1);

    var hvwt = diffwt;
    if (0.5 - varwt) * (0.5 - diffwt) > 0.0 && abs(0.5 - diffwt) < abs(0.5 - varwt) {
        hvwt = varwt;
    }
    st(HVWT, i, hvwt);
}

// Pass 5: the Nyquist texture test at red and blue sites, and the box
// of the marked sites.
@compute @workgroup_size(16, 16)
fn nyquist_test(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(6) || is_green() {
        return;
    }
    let test = GAUSS_ODD[0] * b(CDDIFFSQ, 0)
        + GAUSS_ODD[1] * (b(CDDIFFSQ, -M1) + b(CDDIFFSQ, P1) + b(CDDIFFSQ, -P1) + b(CDDIFFSQ, M1))
        + GAUSS_ODD[2] * (b(CDDIFFSQ, -V2) + b(CDDIFFSQ, -2) + b(CDDIFFSQ, 2) + b(CDDIFFSQ, V2))
        + GAUSS_ODD[3] * (b(CDDIFFSQ, -M2) + b(CDDIFFSQ, P2) + b(CDDIFFSQ, -P2) + b(CDDIFFSQ, M2))
        - (GAUSS_GRAD[0] * b(DELHVSQSUM, 0)
            + GAUSS_GRAD[1] * (b(DELHVSQSUM, -V1) + b(DELHVSQSUM, 1) + b(DELHVSQSUM, -1)
                + b(DELHVSQSUM, V1))
            + GAUSS_GRAD[2] * (b(DELHVSQSUM, -M1) + b(DELHVSQSUM, P1) + b(DELHVSQSUM, -P1)
                + b(DELHVSQSUM, M1))
            + GAUSS_GRAD[3] * (b(DELHVSQSUM, -V2) + b(DELHVSQSUM, -2) + b(DELHVSQSUM, 2)
                + b(DELHVSQSUM, V2))
            + GAUSS_GRAD[4] * (b(DELHVSQSUM, -V2 - 1) + b(DELHVSQSUM, -V2 + 1)
                + b(DELHVSQSUM, -V1 - 2) + b(DELHVSQSUM, -V1 + 2) + b(DELHVSQSUM, V1 - 2)
                + b(DELHVSQSUM, V1 + 2) + b(DELHVSQSUM, V2 - 1) + b(DELHVSQSUM, V2 + 1))
            + GAUSS_GRAD[5] * (b(DELHVSQSUM, -M2) + b(DELHVSQSUM, P2) + b(DELHVSQSUM, -P2)
                + b(DELHVSQSUM, M2)));
    if test > 0.0 {
        st(NYQUIST, i, 1.0);
        let base = tile * 4u;
        atomicMin(&boxes[base], u32(rr));
        atomicMax(&boxes[base + 1u], u32(rr));
        atomicMin(&boxes[base + 2u], u32(cc));
        atomicMax(&boxes[base + 3u], u32(cc));
    }
}

// Pass 5: the majority filter over the eight same-class neighbors.
@compute @workgroup_size(16, 16)
fn nyquist_majority(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || is_green() || !in_box(nyquist_box()) {
        return;
    }
    let count = b(NYQUIST, -V2) + b(NYQUIST, -M1) + b(NYQUIST, P1) + b(NYQUIST, -2)
        + b(NYQUIST, 2) + b(NYQUIST, -P1) + b(NYQUIST, M1) + b(NYQUIST, V2);
    var v = b(NYQUIST, 0);
    if count > 4.0 {
        v = 1.0;
    } else if count < 4.0 {
        v = 0.0;
    }
    st(NYQUIST2, i, v);
}

// Pass 5: the weight by area statistics in Nyquist regions.
@compute @workgroup_size(16, 16)
fn nyquist_area(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || is_green() || !in_box(nyquist_box()) || ld(NYQUIST2, i) == 0.0 {
        return;
    }
    var sumcfa = 0.0;
    var sumh = 0.0;
    var sumv = 0.0;
    var sumsqh = 0.0;
    var sumsqv = 0.0;
    var areawt = 0.0;
    for (var di = -6; di < 7; di += 2) {
        for (var dj = -6; dj < 7; dj += 2) {
            let j = i + di * TS + dj;
            if ld(NYQUIST2, j) == 0.0 {
                continue;
            }
            let c = ld(CFA, j);
            let l = ld(CFA, j - 1);
            let r = ld(CFA, j + 1);
            let u = ld(CFA, j - V1);
            let d = ld(CFA, j + V1);
            sumcfa += c;
            sumh += l + r;
            sumv += u + d;
            sumsqh += sq(c - l) + sq(c - r);
            sumsqv += sq(c - u) + sq(c - d);
            areawt += 1.0;
        }
    }
    sumh = sumcfa - 0.5 * sumh;
    sumv = sumcfa - 0.5 * sumv;
    areawt *= 0.5;
    let hcdvar = EPS_SQ + abs(areawt * sumsqh - sumh * sumh);
    let vcdvar = EPS_SQ + abs(areawt * sumsqv - sumv * sumv);
    st(HVWT, i, hcdvar / (vcdvar + hcdvar));
}

// Pass 6: green at red and blue sites. The weight at a cell reads its
// four diagonal neighbors, the row above already rewritten: a
// workgroup a tile, a thread a column, the rows in order.
@compute @workgroup_size(160)
fn green_at_chroma(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    set_tile(wid.x);
    let rows = geometry(wid.x).z;
    let c = i32(lid.x);
    for (var r = 8; r < rows - 8; r++) {
        set_cell(r, c);
        if c >= 8 && c < cc1 - 8 && !is_green() {
            let alt = 0.25 * (b(HVWT, -M1) + b(HVWT, P1) + b(HVWT, -P1) + b(HVWT, M1));
            var hvwt = b(HVWT, 0);
            if abs(0.5 - hvwt) < abs(0.5 - alt) {
                hvwt = alt;
            }
            st(HVWT, i, hvwt);
            let dgrb0 = hvwt * ld(VCD, i) + (1.0 - hvwt) * ld(HCD, i);
            st(DGRB0, i, dgrb0);
            st(GREEN, i, ld(CFA, i) + dgrb0);
        }
        storageBarrier();
    }
}

// Pass 6: green curvature inside Nyquist regions.
@compute @workgroup_size(16, 16)
fn curvature(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(8) || is_green() || ld(NYQUIST2, i) == 0.0 {
        return;
    }
    let g0 = b(GREEN, 0);
    st(DGRB2H, i, sq(g0 - 0.5 * (b(GREEN, -1) + b(GREEN, 1))));
    st(DGRB2V, i, sq(g0 - 0.5 * (b(GREEN, -V1) + b(GREEN, V1))));
}

fn quinc(p: u32) -> f32 {
    return EPS_SQ + G_QUINC[0] * b(p, 0) + G_QUINC[1] * (b(p, -M1) + b(p, P1) + b(p, -P1) + b(p, M1))
        + G_QUINC[2] * (b(p, -V2) + b(p, -2) + b(p, 2) + b(p, V2))
        + G_QUINC[3] * (b(p, -M2) + b(p, P2) + b(p, -P2) + b(p, M2));
}

// Pass 6: the refinement of green by its curvature in Nyquist regions.
@compute @workgroup_size(16, 16)
fn refine(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || is_green() || !in_box(nyquist_box()) || ld(NYQUIST2, i) == 0.0 {
        return;
    }
    let gvarh = quinc(DGRB2H);
    let gvarv = quinc(DGRB2V);
    let dgrb0 = (ld(HCD, i) * gvarv + ld(VCD, i) * gvarh) / (gvarv + gvarh);
    st(DGRB0, i, dgrb0);
    st(GREEN, i, ld(CFA, i) + dgrb0);
}

// Pass 7: diagonal gradients at red and blue sites, squared diagonal
// differences at green sites.
@compute @workgroup_size(16, 16)
fn diagonals(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(6) {
        return;
    }
    if is_green() {
        st(DGRBSQ1P, i, sq(s(0) - s(-P1)) + sq(s(0) - s(P1)));
        st(DGRBSQ1M, i, sq(s(0) - s(-M1)) + sq(s(0) - s(M1)));
    } else {
        st(DELP, i, abs(s(P1) - s(-P1)));
        st(DELM, i, abs(s(M1) - s(-M1)));
    }
}

fn diag_est(cr: f32, near: f32, far: f32) -> f32 {
    if abs(1.0 - cr) < AR_THRESH {
        return s(0) * cr;
    }
    return near + 0.5 * (s(0) - far);
}

fn even(p: u32) -> f32 {
    return EPS_SQ + GAUSS_EVEN[0] * (b(p, -V1) + b(p, -1) + b(p, 1) + b(p, V1))
        + GAUSS_EVEN[1] * (b(p, -V2 - 1) + b(p, -V2 + 1) + b(p, -2 - V1) + b(p, 2 - V1)
            + b(p, -2 + V1) + b(p, 2 + V1) + b(p, V2 - 1) + b(p, V2 + 1));
}

fn diag_bound(v_in: f32, n: f32, f: f32) -> f32 {
    var v = v_in;
    let s0v = s(0);
    if v < s0v {
        if 2.0 * v < s0v {
            v = median(v, n, f);
        } else {
            let wt = 2.0 * (s0v - v) / (EPS + v + s0v);
            v = wt * v + (1.0 - wt) * median(v, n, f);
        }
    }
    if v > CLIP_PT {
        v = median(v, n, f);
    }
    return v;
}

// Pass 8: the other chroma at red and blue sites along diagonals.
@compute @workgroup_size(16, 16)
fn chroma_diagonal(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(8) || is_green() {
        return;
    }
    let crse = 2.0 * s(M1) / (EPS + s(0) + s(M2));
    let crnw = 2.0 * s(-M1) / (EPS + s(0) + s(-M2));
    let crne = 2.0 * s(P1) / (EPS + s(0) + s(P2));
    let crsw = 2.0 * s(-P1) / (EPS + s(0) + s(-P2));
    let rbse = diag_est(crse, s(M1), s(M2));
    let rbnw = diag_est(crnw, s(-M1), s(-M2));
    let rbne = diag_est(crne, s(P1), s(P2));
    let rbsw = diag_est(crsw, s(-P1), s(-P2));

    let wtse = EPS + b(DELM, 0) + b(DELM, M1) + b(DELM, M2);
    let wtnw = EPS + b(DELM, 0) + b(DELM, -M1) + b(DELM, -M2);
    let wtne = EPS + b(DELP, 0) + b(DELP, P1) + b(DELP, P2);
    let wtsw = EPS + b(DELP, 0) + b(DELP, -P1) + b(DELP, -P2);

    let rbm = (wtse * rbnw + wtnw * rbse) / (wtse + wtnw);
    let rbp = (wtne * rbsw + wtsw * rbne) / (wtne + wtsw);

    let rbvarm = even(DGRBSQ1M);
    let rbvarp = even(DGRBSQ1P);
    st(PMWT, i, rbvarm / (rbvarp + rbvarm));

    st(RBP, i, diag_bound(rbp, s(-P1), s(P1)));
    st(RBM, i, diag_bound(rbm, s(-M1), s(M1)));
}

// Pass 9: the diagonal weight refined from its neighbors, as pass 6's
// (the row above already rewritten), and the R+B estimate.
@compute @workgroup_size(160)
fn diagonal_weight(
    @builtin(local_invocation_id) lid: vec3<u32>,
    @builtin(workgroup_id) wid: vec3<u32>,
) {
    set_tile(wid.x);
    let rows = geometry(wid.x).z;
    let c = i32(lid.x);
    for (var r = 10; r < rows - 10; r++) {
        set_cell(r, c);
        if c >= 10 && c < cc1 - 10 && !is_green() {
            let alt = 0.25 * (b(PMWT, -M1) + b(PMWT, P1) + b(PMWT, -P1) + b(PMWT, M1));
            var pmwt = b(PMWT, 0);
            if abs(0.5 - pmwt) < abs(0.5 - alt) {
                pmwt = alt;
                st(PMWT, i, pmwt);
            }
            st(RBINT, i, 0.5 * (ld(CFA, i) + ld(RBM, i) * (1.0 - pmwt) + ld(RBP, i) * pmwt));
        }
        storageBarrier();
    }
}

fn rb_est(cr: f32, near: f32, far: f32) -> f32 {
    if abs(1.0 - cr) < AR_THRESH {
        return b(RBINT, 0) * cr;
    }
    return near + 0.5 * (b(RBINT, 0) - far);
}

fn rb_bound(v_in: f32, n: f32, f: f32) -> f32 {
    var v = v_in;
    let rb0 = b(RBINT, 0);
    if v < rb0 {
        if 2.0 * v < rb0 {
            v = median(v, n, f);
        } else {
            let wt = 2.0 * (rb0 - v) / (EPS + v + rb0);
            v = wt * v + (1.0 - wt) * median(v, n, f);
        }
    }
    return v;
}

// Pass 10's body: where the diagonal weight is more decisive than the
// axis weight, green again from the R+B estimate.
fn rederive_green() {
    if !inside(12) || is_green() {
        return;
    }
    if abs(0.5 - ld(PMWT, i)) < abs(0.5 - ld(HVWT, i)) {
        return;
    }
    let cru = s(-V1) * 2.0 / (EPS + b(RBINT, 0) + b(RBINT, -V2));
    let crd = s(V1) * 2.0 / (EPS + b(RBINT, 0) + b(RBINT, V2));
    let crl = s(-1) * 2.0 / (EPS + b(RBINT, 0) + b(RBINT, -2));
    let crr = s(1) * 2.0 / (EPS + b(RBINT, 0) + b(RBINT, 2));
    let gu = rb_est(cru, s(-V1), b(RBINT, -V2));
    let gd = rb_est(crd, s(V1), b(RBINT, V2));
    let gl = rb_est(crl, s(-1), b(RBINT, -2));
    let gr = rb_est(crr, s(1), b(RBINT, 2));

    var gintv = (b(DIRWTS0, -V1) * gd + b(DIRWTS0, V1) * gu) / (b(DIRWTS0, V1) + b(DIRWTS0, -V1));
    var ginth = (b(DIRWTS1, -1) * gr + b(DIRWTS1, 1) * gl) / (b(DIRWTS1, -1) + b(DIRWTS1, 1));

    gintv = rb_bound(gintv, s(-V1), s(V1));
    ginth = rb_bound(ginth, s(-1), s(1));
    if ginth > CLIP_PT {
        ginth = median(ginth, s(-1), s(1));
    }
    if gintv > CLIP_PT {
        gintv = median(gintv, s(-V1), s(V1));
    }
    let hvwt = ld(HVWT, i);
    let green = ginth * (1.0 - hvwt) + gintv * hvwt;
    st(GREEN, i, green);
    st(DGRB0, i, green - ld(CFA, i));
}

// Passes 10 and 11: green re-derived, then G-B split out of G-R at
// blue sites. Each reads and writes its own cell alone across the two.
@compute @workgroup_size(16, 16)
fn green_again(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) {
        return;
    }
    rederive_green();
    if inside(8) && color_at(rr, cc) == 2u {
        st(DGRB1, i, ld(DGRB0, i));
        st(DGRB0, i, 0.0);
    }
}

// Pass 12: the missing chroma difference at red and blue sites from
// the four diagonal neighbors of the other color.
@compute @workgroup_size(16, 16)
fn other_chroma(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(14) {
        return;
    }
    let color = color_at(rr, cc);
    if color == 1u {
        return;
    }
    // At a blue site the G-R plane, at a red one the G-B.
    var p = DGRB0;
    if color == 0u {
        p = DGRB1;
    }
    let wtnw = 1.0 / (EPS + abs(b(p, -M1) - b(p, M1)) + abs(b(p, -M1) - b(p, -M3))
        + abs(b(p, M1) - b(p, -M3)));
    let wtne = 1.0 / (EPS + abs(b(p, P1) - b(p, -P1)) + abs(b(p, P1) - b(p, P3))
        + abs(b(p, -P1) - b(p, P3)));
    let wtsw = 1.0 / (EPS + abs(b(p, -P1) - b(p, P1)) + abs(b(p, -P1) - b(p, M3))
        + abs(b(p, P1) - b(p, -P3)));
    let wtse = 1.0 / (EPS + abs(b(p, M1) - b(p, -M1)) + abs(b(p, M1) - b(p, -P3))
        + abs(b(p, -M1) - b(p, M3)));
    let value = (wtnw * (1.325 * b(p, -M1) - 0.175 * b(p, -M3) - 0.075 * b(p, -M1 - 2)
        - 0.075 * b(p, -M1 - V2))
        + wtne * (1.325 * b(p, P1) - 0.175 * b(p, P3) - 0.075 * b(p, P1 + 2)
            - 0.075 * b(p, P1 + V2))
        + wtsw * (1.325 * b(p, -P1) - 0.175 * b(p, -P3) - 0.075 * b(p, -P1 - 2)
            - 0.075 * b(p, -P1 - V2))
        + wtse * (1.325 * b(p, M1) - 0.175 * b(p, M3) - 0.075 * b(p, M1 + 2)
            - 0.075 * b(p, M1 + V2)))
        / (wtnw + wtne + wtsw + wtse);
    st(p, i, value);
}

fn chroma_at_green(p: u32, g: f32, temp: f32) -> f32 {
    return g - (b(HVWT, -V1) * b(p, -V1) + (1.0 - b(HVWT, 1)) * b(p, 1)
        + (1.0 - b(HVWT, -1)) * b(p, -1) + b(HVWT, V1) * b(p, V1)) * temp;
}

// The tile's interior into the band: red and blue from green minus the
// chrominance, weighted over the cardinal neighbors at green sites.
@compute @workgroup_size(16, 16)
fn output(@builtin(global_invocation_id) id: vec3<u32>) {
    if !enter(id) || !inside(MARGIN) {
        return;
    }
    let g = ld(GREEN, i);
    var r: f32;
    var bl: f32;
    if is_green() {
        let temp = 1.0 / (b(HVWT, -V1) + 2.0 - b(HVWT, 1) - b(HVWT, -1) + b(HVWT, V1));
        r = chroma_at_green(DGRB0, g, temp);
        bl = chroma_at_green(DGRB1, g, temp);
    } else {
        r = g - ld(DGRB0, i);
        bl = g - ld(DGRB1, i);
    }
    let y = u32(top + rr) - P.band_y0;
    let x = u32(left + cc);
    let o = (y * P.size.x + x) * 3u;
    band[o] = clamp(r, 0.0, P.ceiling);
    band[o + 1u] = clamp(g, 0.0, P.ceiling);
    band[o + 2u] = clamp(bl, 0.0, P.ceiling);
}
