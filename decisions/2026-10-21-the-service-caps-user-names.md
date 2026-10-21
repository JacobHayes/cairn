# The service caps how many users one read names

Question (from the work making what agents are told agree with itself): the cap of a page of
users per name read was enforced only by the HTTP handler, the in-browser host named any
number, the API document spelled the number out in two descriptions, and the web app kept its
own copy of it. Where does the cap live?

Call: the service's name read refuses more than the page limit with a read error the caller
can change; the API answers it as a bad request, the in-browser host as refused, and the tools
as refused. The API document serves the limit from the constant: the repeated user parameter
carries it as its most items, and the page size parameter as its maximum, so no description
spells the number. The generated rule tables carry the page limit, which the web app batches
its name reads by. The store's own copy of the page limit is now the schema's.

Alternatives: keeping the handler's check and adding one to the in-browser host (two checks
of one rule); cutting an oversized ask to a page instead of refusing it (a caller would get
fewer names than it asked for without being told).

What would change it: a caller that needs more names at once than a page, which would be a
case for paging the name read rather than raising the limit.
