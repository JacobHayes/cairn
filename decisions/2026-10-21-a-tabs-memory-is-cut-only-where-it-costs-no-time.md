# A tab's memory is cut only where it costs no time

**Question.** Which cuts to a tab's fixed memory are worth taking?

**Call.** A cut lands only when it measurably saves memory and costs no time a person feels: the mean derive at normal sizes (the bundled journeys, and documents up to half the node limit) stays under 10 ms more. By that rule:
- The engine has one instance per tab, in the derive worker, which answers every engine call the page makes, route files included.
- The wasm module is built for size: `opt-level = "s"` then `wasm-opt -Oz`. It saves 11 to 15 MB of a tab and costs about 4 ms at half the node limit.
- The layout worker starts at boot. Starting it on demand would save about 20 MB but makes a tab's first graph wait about 45 ms longer.

**Alternatives.** Level 3 (more memory for derive time no one feels); `z` with `-Oz` (a little more memory, a derive 16 to 28 percent slower from 500 nodes up); the layout worker on demand (a felt wait).

**What would change it.** A derive budget that counts time at half the node limit, or many large journeys derived at once (back to level 3); memory becoming the tighter budget (the layout worker on demand, then `z`).
