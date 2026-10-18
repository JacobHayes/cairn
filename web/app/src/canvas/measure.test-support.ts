// The text measure the unit tests run on (Node has no fonts): capitals and digits wide, the rest
// average, a space narrow. The browser tests measure the real face (measure.ts).
import { setEmWidth } from "./measure.ts";

const CHAR_EM = 0.58;
const WIDE_CHAR_EM = 0.78;
const SPACE_EM = 0.3;

setEmWidth((text) => Array.from(text).reduce((sum, char) => sum + (char === " " ? SPACE_EM : /[\p{Lu}\p{N}]/u.test(char) ? WIDE_CHAR_EM : CHAR_EM), 0));
