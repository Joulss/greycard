# §647. Model URLs pinned to commits (2026-11-12)

An outside review found every model URL in `greycard-ai::registry` on
`resolve/main`, a branch that moves with every upload. A file's size
and SHA-256 were already in the registry and checked after each
download, so a changed file failed loudly rather than loaded; but a
fixed release could still stop working the day a repository moved on.
Each URL now names the commit we checked it at,
`resolve/<40-hex commit>/<file>`, and a test
(`every_hugging_face_url_names_a_commit`) refuses any Hugging Face URL
in the models, the Core ML files, or the retired files that names a
branch.

We read each repository's `main` commit and file list from the public
API on 2026-11-12 and compared every file's LFS oid and size with the
registry: all 31 match. Two of the remove's constants,
`text_embeds.f16` and `time_ids.f16`, are plain git files with no LFS
oid, so we fetched those two (2,572 bytes in all) at the commit; both
SHA-256s and git blob oids match. No file lacked a hash; no store
directory changes, so no model is fetched again.

| Repository | File | Commit | Bytes | SHA-256 |
|---|---|---|---|---|
| onnx-community/BiRefNet_lite-ONNX | onnx/model_fp16.onnx | de15b22ba131738a16dff04aab8bdf8dc32e3ac1 | 114538221 | d39b897ceb16ae654c1731f3dba0cf9b368d9cae74b5a57459b455cc8bfec402 |
| jessolmstead/BiRefNet_lite-ONNX-webgpu | model_fp16_webgpu.onnx | 0333e93b8f3f76f89ae7c6942b0d53ac1ed21318 | 113778088 | 0a019d6ba73c9cedc9a251f8c9390b196ff6399acd281a2872692861abbd78c2 |
| onnx-community/sam2.1-hiera-small-ONNX | onnx/vision_encoder.onnx | a7df49d8de14b9d2e4504d1687b0d568f905fd8d | 467440 | aacf1f7137bb6fffcf6bf166abcfabe28f57a76059254f3fb611c4a64a208119 |
| onnx-community/sam2.1-hiera-small-ONNX | onnx/vision_encoder.onnx_data | a7df49d8de14b9d2e4504d1687b0d568f905fd8d | 162476288 | 260fd1f0a34e72a3dc79a739e563b4facc0ba75504818b433a1f808e66637456 |
| onnx-community/sam2.1-hiera-small-ONNX | onnx/prompt_encoder_mask_decoder.onnx | a7df49d8de14b9d2e4504d1687b0d568f905fd8d | 213114 | 079c59b261f723ff5c6a125e69b0170a957b21c58738c28d2b0394ecd0587d7f |
| onnx-community/sam2.1-hiera-small-ONNX | onnx/prompt_encoder_mask_decoder.onnx_data | a7df49d8de14b9d2e4504d1687b0d568f905fd8d | 20958208 | f9e59a584ab8ced21fa812c211bc01084204db1c9e92a5ef4fb3a49972b4e864 |
| jessolmstead/greycard-sam3 | sam3_image_encoder_fp16.onnx | 1e8b05a723c0dfa07b932c6748c75dd77b7cbaa3 | 908762234 | dd4733a6882264d0ab00599acd6ece3dd700f9b61b7976369b956ce41d88c23d |
| jessolmstead/greycard-sam3 | sam3_decoder.onnx | 1e8b05a723c0dfa07b932c6748c75dd77b7cbaa3 | 129854990 | 7c205232b0698d532731fd5fa206cdea694717de69c0d8e88997a14027425564 |
| jessolmstead/greycard-sam3 | sam3_presets.bin | 1e8b05a723c0dfa07b932c6748c75dd77b7cbaa3 | 524962 | a1c205ef7e4374a6a9072928339d957eb78f78efa01b5cf170fe0f6ff92d6917 |
| jessolmstead/greycard-sky | eomt-s-coco-640.onnx | c30732066a8e738b971a458906af01ca02174728 | 96025182 | 805ed0fb363784811a7f3979335c2c64a7d40f91ae943326f79a56ff477f372d |
| jessolmstead/greycard-lama | big-lama-dynamic-fp32.onnx | 935b224346521b6202d45d482b82a04b30d9c92d | 209024270 | 5f39f5e80b91596fd50f3227858c97912dabc0ffbd4796f21ca688529abb8771 |
| jessolmstead/greycard-rorem | unet-1024.onnx | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 2521003 | d81aed3cc00059e60e7c476be8aa679a9c78e3872cc73848bdd01b02c4b5fdf2 |
| jessolmstead/greycard-rorem | unet-512.onnx | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 2520999 | 0cfff605ed573a5695ccee47b91fec586b03464f421c8ceb37834db2218e9796 |
| jessolmstead/greycard-rorem | unet.onnx_data | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 5134931840 | 3c883667b7e45d131d0d45d1339e28d38dfa3c210a3258f2afb637e58cfe50a5 |
| jessolmstead/greycard-rorem | vae-encoder-512.onnx | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 68427415 | 0d5368ce83a6a6a5cdcac5ffbb082f5102b9557441e7df8436949a0648290ca1 |
| jessolmstead/greycard-rorem | vae-encoder-1024.onnx | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 68427415 | 8175c93cb33d553325ce940be504a61adca76ec0fbe1ed41c8e5bc46402044e6 |
| jessolmstead/greycard-rorem | vae-decoder-512.onnx | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 99084062 | 2acf1e075627ec41e68a243f048a5fd0bcee57548dc3b41eef4e5d101b2d8e6a |
| jessolmstead/greycard-rorem | vae-decoder-1024.onnx | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 99084064 | 25bcb399f90e732cac2fa8d28c15b61acf2872f095737c870df99160b34e1a7f |
| jessolmstead/greycard-rorem | prompt_embeds.f16 | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 315392 | 739efee6b5c6deb2c4ecf3d1bf8908242e9e1a5b27ae8896d16fc904873b2eb0 |
| jessolmstead/greycard-rorem | text_embeds.f16 (git, not LFS) | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 2560 | c2e5c4dff874db93dc53e5cc78d9c3fb3789f1342738729eb755c9fe5a2b82fa |
| jessolmstead/greycard-rorem | time_ids.f16 (git, not LFS) | 1d63598ef6ff2d1a08ffb0a7b8eb6aa5cd563896 | 12 | 4c84e226abda27b53afd64b62938667278fc0d8b0babc9bbb0c384b49cde554c |
| jessolmstead/greycard-rorem-fast | unet-lcm-1024.onnx | b3551ec94e811043db42dae7ee9fdb11a9e6dc49 | 3419476 | 0b5320148e08fc354f2b4e6b1f2f1927d8fb3c8bf2f522188db193d3ad90f4f2 |
| jessolmstead/greycard-rorem-fast | unet-lcm-512.onnx | b3551ec94e811043db42dae7ee9fdb11a9e6dc49 | 3419472 | c12747ce96160ec6ad0c89a941fc70452ad8784d563594183a18a5cb905bfc62 |
| jessolmstead/greycard-rorem-fast | unet-lora.onnx_data | b3551ec94e811043db42dae7ee9fdb11a9e6dc49 | 393502720 | 2beaa20d700c59e191e51fd1d9a0698110a9354c6ec16b7c570ba96e7bff6d33 |
| jessolmstead/greycard-denoise | denoise-v19.onnx | 0b468a819b7eb3e40280b52d1475713bff3d373b | 4932834 | 319d024a39d3e31c95e99c91bb5c089691707cbf13f4f077c1c6e49d10235c49 |
| jessolmstead/greycard-denoise | denoise-v15.onnx | 0b468a819b7eb3e40280b52d1475713bff3d373b | 9037296 | f5845119e0b0ee63c57aad38129aed1c8925e60ec9b7838c6b7a0fa23a84a546 |
| jessolmstead/greycard-denoise | denoise-v20.onnx | 0b468a819b7eb3e40280b52d1475713bff3d373b | 20297204 | c20aed71a45b2a984570e09d8456c54ce8eff43a334b54c7ebb480c21518f9be |
| jessolmstead/greycard-denoise | denoise-v19-fp16.onnx | 0b468a819b7eb3e40280b52d1475713bff3d373b | 2476627 | a7b6df9f9e61cdb81c8a2413cf001e2d0a49f3c9c3645e97b28232f48b554b54 |
| jessolmstead/greycard-denoise | denoise-v15-fp16.onnx | 0b468a819b7eb3e40280b52d1475713bff3d373b | 4528894 | 53b96629dd6185e6a02f9fcab97e569a73bc1056a198bd2cdc466b128f3e214c |
| jessolmstead/greycard-denoise | denoise-v20-fp16.onnx | 0b468a819b7eb3e40280b52d1475713bff3d373b | 10158848 | bf9f584ba6a30114dd9df74854cadf0125ce760e39251d30835312a4c81f4dd7 |
| Carve/LaMa-ONNX (retired) | lama_fp32.onnx | c3c0c9e468934d62e79c329e35d82dd09ff8c444 | 208044816 | 1faef5301d78db7dda502fe59966957ec4b79dd64e16f03ed96913c7a4eb68d6 |

Third-party repositories, pinned the same way, with the licenses the
registry gives:

- `onnx-community/BiRefNet_lite-ONNX`: BiRefNet lite, MIT.
- `onnx-community/sam2.1-hiera-small-ONNX`: SAM 2.1, Apache-2.0.
- `Carve/LaMa-ONNX`: retired; its URL is only ever printed in an old
  store's note, since the store matches a retired file by name, size,
  and hash and never fetches it. Apache-2.0 (LaMa).

The rest are ours, under `jessolmstead/`.

Updating a model from now on: upload the file, read the new commit
from `https://huggingface.co/api/models/<repo>` (`sha`) and the file's
size and LFS oid from `.../tree/<sha>`, then put all three in the
registry (commit in the URL, `bytes`, `sha256`) and change the model's
`id` so the store fetches it fresh. An upload alone changes nothing
for any release. A small file that Hugging Face keeps in git rather
than LFS has no oid in the listing; hash it locally.

Left alone: the RAIL++-M license link
(`stabilityai/stable-diffusion-xl-base-1.0/blob/main/LICENSE.md`) is a
page we cite, not a file we fetch, and the license text ships in
`rorem_license.txt`. The notes (§70, §159, §175, §253, §256) still name
the repositories and one `resolve/main` URL as history.
