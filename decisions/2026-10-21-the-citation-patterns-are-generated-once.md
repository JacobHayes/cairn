# What served and shown text never cites is one generated pattern

Question (from the work making what agents are told agree with itself): the rule that text a
person or an agent reads holds no requirement id and no name of one of the repository's
documents was implemented twice: a byte scanner in the integration tests and a regular
expression in the browser tests, each reading the PRD's section headings and the repository's
root itself. The browser check also missed a key a screen sets in capitals with CSS. Where does
the rule live?

Call: the rule-table generator writes the two patterns into the generated rule tables, from
the PRD's `### X.` headings and the capitalised Markdown files at the root, as ECMAScript
regular expressions. The browser tests compile them as they are; the integration tests read
them from the same file and match them as JSON Schema patterns, which are ECMAScript regular
expressions, through the schema validator they already use, so no regular-expression
dependency is added. A change to the PRD's sections or the root's documents shows up as a
stale generated file. The browser check matches keys and digests in any case, since what a
person reads is the text as styled.

Alternatives: a separate generated data file for the tests only (a second generated artifact
for two strings); keeping both implementations and testing they agree (two homes kept in
step); reading the page's raw text instead of the styled text (it includes hidden text, which
would flag what no person sees).

What would change it: a third reader of the rule outside the tests, such as a lint over
documents, which might want the generator to own a small library instead.
