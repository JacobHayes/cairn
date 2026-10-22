# The web app is sent compressed

A browser that accepts brotli or gzip now gets the app's files compressed.
Every release changes the bundles' hashes, so each person's first load after
a deploy downloads all of it; it is now a fraction of what it was.

Bytes sent for one request each, from the server run on a fixture build
(`curl` with the named `Accept-Encoding`):

| File | Before (nothing compressed) | gzip | brotli |
|---|---|---|---|
| `index.html` | 2,840 | 889 | 715 |
| main script (`assets/index-*.js`) | 302,729 | 90,303 | 75,994 |
| engine module (`assets/*.wasm`, local unoptimized build) | 11,170,559 | 2,238,644 | 1,319,911 |

The deployed engine module is 5.7 MB uncompressed, so about a fifth of that
sent as brotli.

- Brotli is preferred, then gzip, else the file as it is; a client that
  refuses a coding (`br;q=0`) is not sent it.
- Responses for files that have a compressed form say `Vary: Accept-Encoding`
  so a cache keeps the forms apart.
- Compression is done once, when the app is built, so the server spends no
  time on it per request.

Known limits:

- The static demo site (`build:web demo`) is not precompressed; its host
  compresses what it serves. `cairn demo` serves the compressed build.
- Images and fonts that are already compressed are sent as they are.
