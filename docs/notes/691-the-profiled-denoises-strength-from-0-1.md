# §691. The profiled denoise's Strength from 0.1 (2026-11-15)

A tester found the profiled denoise soft with Strength at its lowest,
0.5. Measured on seven frames from five makers, 0.5 already takes 50 to
70% of the fine grain with every edge kept, and edges start to go at
1.0: the range began most of the way to its top.

Strength divides the non-local means' patch dissimilarity, and the
means trade well below 0.5. On an ISO 1600 frame, 0.25 keeps 59% of the
grain and every edge, where a half mix with the original keeps 54% of
the grain and 92% of the edge detail. So the range runs lower instead
of gaining a mix.

The slider now runs from 0.1 to 3, the default still 1.5.
`Noise::STRENGTH_MIN` and `STRENGTH_MAX` name the range, and a test
holds the panel's slider to them. The engine is untouched, and nothing
ever clamped a stored value, so an edit at 0.5 or above renders as it
did. 0.1 is safe in the means: they divide by the strength floored at
1e-3, and a pixel always has full weight on itself
(`a_lower_strength_denoises_less`).

A Lightroom Luminance under 25 was floored at 0.4 on import; it now
maps down to 0.1 (10 gives 0.2). Only a fresh import sees it.

Left for later, on the roadmap: the wavelet stage runs at its own
strength whatever the slider says, taking a quarter to a third of the
2 to 6 px mottling at any setting.
