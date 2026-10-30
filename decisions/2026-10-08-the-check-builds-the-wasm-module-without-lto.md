# The local check builds the engine's wasm module without LTO, and CI builds it as it ships

Question: the optimized wasm module (release, LTO, one codegen unit) costs most of a minute to link after any engine change. Does the local check need it, and do the committed bindings depend on it?

Call: the bindings depend on stripping, not on LTO, codegen units, or opt-level: an unstripped module keeps its target-features section, and wasm-bindgen then binds differently. So a fast stripped profile without LTO is the local default for building the module, and the release build and CI use the optimized one for the whole ladder. On every push CI checks the committed bindings against the module as it ships and runs the browser tests on it, so a difference between the two profiles fails CI and never passes silently.

Alternatives: generating bindings from the fast build while also building the optimized one (still pays the link); opt-level 3 without LTO (slower to build, no faster in the tests).

What would change it: a wasm-bindgen version whose output depends on more than stripping, or a browser test that fails only on the optimized module; CI catches either.
