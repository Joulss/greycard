//! Sums taken in parallel that come out the same to the bit however
//! the threads share the work.
//!
//! Rayon's `sum` and `reduce` add the parts where its work-stealing
//! happened to split the work, and floating-point addition is not
//! associative, so one plane can sum to a different last bit from one
//! run to the next: a machine busy with something else steals
//! differently. A mean that then sets a threshold moves the pixels
//! near it by a level. Here the parts are fixed before the threads
//! start (a piece of the plane, or a row), each is summed in order on
//! whichever thread takes it, and the parts are added in order.
//!
//! The least and the most need none of this: they are the same in any
//! order.

use rayon::prelude::*;

/// Values a piece of a plane's sum. Large enough that a piece is worth
/// a task and the pieces' own sums are few (some 700 for a 45 MP
/// plane), small enough that a plane of a few megapixels still spreads
/// over the threads. Fixed, never taken from the thread count, since
/// where the pieces end is what the sum's last bit depends on.
pub const CHUNK: usize = 1 << 16;

/// The parts, folded into `init` in index order however the threads
/// made them. The order is fixed by the parts, so the result is.
pub fn in_order<P, A>(
    parts: impl IndexedParallelIterator<Item = P>,
    init: A,
    add: impl FnMut(A, P) -> A,
) -> A
where
    P: Send,
{
    parts.collect::<Vec<P>>().into_iter().fold(init, add)
}

/// The sum of `term` over `values` in double: pieces of [`CHUNK`]
/// summed in parallel, each in order, and the pieces' sums in order.
pub fn sum_f64<T: Sync>(values: &[T], term: impl Fn(&T) -> f64 + Sync) -> f64 {
    in_order(
        values
            .par_chunks(CHUNK)
            .map(|piece| piece.iter().map(&term).sum::<f64>()),
        0.0,
        |total, piece| total + piece,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Terms of mixed size and sign, so that the order of the adds
    /// shows in the last bit.
    fn terms(n: usize) -> Vec<f32> {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        (0..n)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let unit = (state >> 40) as f32 / (1u64 << 24) as f32;
                let scale = [1e-6, 1.0, 1e4][(state & 3) as usize % 3];
                (unit - 0.3) * scale
            })
            .collect()
    }

    fn pool(threads: usize) -> rayon::ThreadPool {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("a test pool")
    }

    /// The serial sum the parallel one must equal: the same pieces,
    /// one after another on one thread.
    fn serial(values: &[f32]) -> f64 {
        let mut total = 0.0;
        for piece in values.chunks(CHUNK) {
            total += piece.iter().map(|&v| v as f64).sum::<f64>();
        }
        total
    }

    #[test]
    fn a_sum_is_the_serial_sum_to_the_bit_at_any_thread_count() {
        // Several pieces and a short last one.
        let values = terms(7 * CHUNK + 1234);
        let want = serial(&values);
        for threads in [1, 4] {
            for _ in 0..8 {
                let got = pool(threads).install(|| sum_f64(&values, |&v| v as f64));
                assert_eq!(got.to_bits(), want.to_bits(), "{threads} threads");
            }
        }
    }

    #[test]
    fn parts_fold_in_index_order_at_any_thread_count() {
        // Rows of a plane, each summed alone, as the highlights' and the
        // dehaze's sums are.
        let (w, h) = (517, 389);
        let values = terms(w * h);
        let want = values
            .chunks(w)
            .map(|row| row.iter().map(|&v| v as f64).sum::<f64>())
            .fold(0.0f64, |a, b| a + b);
        for threads in [1, 4] {
            for _ in 0..8 {
                let got = pool(threads).install(|| {
                    in_order(
                        values
                            .par_chunks(w)
                            .map(|row| row.iter().map(|&v| v as f64).sum::<f64>()),
                        0.0f64,
                        |a, b| a + b,
                    )
                });
                assert_eq!(got.to_bits(), want.to_bits(), "{threads} threads");
            }
        }
    }
}
