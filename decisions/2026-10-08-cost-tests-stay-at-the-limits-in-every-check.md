# Cost tests stay at the limits in every check; their independent derives run side by side

Question: the cost tests are among the slowest local tests. Could they run on smaller inputs locally, with the full size only in CI, as property tests run fewer cases locally?

Call: no. Each checks an absolute operation budget on a generated graph at the schema limits and asserts it is that graph; a budget can hold on a small graph while a superlinear term breaks it at the limit. Property tests differ: fewer cases sample the same space less, not a different space. Independent derives within one cost test run on threads side by side, so a test takes as long as its longest part.

Alternatives: a smaller graph locally and the limits in CI (a weaker local guarantee, and a second size to keep honest); one test per reading (repeats the setup).

What would change it: a cost test written as a comparison of two sizes, which could then run small locally.
