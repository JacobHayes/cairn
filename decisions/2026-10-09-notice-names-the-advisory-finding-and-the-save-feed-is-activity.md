# "Notice" names the advisory finding; the app's save feed is "activity"

Question (from the web redesign and the notices work, PRD A20): the web app's toast store was called notices (`Notices`, `useNotices`), while A20 makes "notice" a domain term for an advisory finding about a route graph. One word for two things invites confusion in code and conversation.

Call: the domain keeps "notice". The app's store becomes `data/activity.ts` (`Activity`, `SaveEvent`, `useActivity`), feeding the sync chip's Recent. A notice's wire shape is `Notice { code: NoticeCode, node: NodeKey, path: Path, message: String }`, with `unanchored` as the one code, so later notices reuse it.

Alternatives: renaming the domain concept ("finding", "advisory"): the PRD term was chosen first and reads well to authors.

What would change it: nothing expected; a second kind of app feed would get its own name.
