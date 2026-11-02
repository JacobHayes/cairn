/** The window width from which the page is a desktop's (the rail beside the page); narrower, it is a phone's. The one place the number is written in code; the stylesheets name it in their media queries, which `breakpoints.test.ts` holds to it. */
export const DESKTOP_MIN = 720;

/** The window width from which the inspector is a column beside the page; narrower, it is a bottom sheet. Written once, as `DESKTOP_MIN` is. */
export const COLUMN_MIN = 1100;
