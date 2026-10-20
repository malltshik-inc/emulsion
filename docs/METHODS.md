# Methods — recommendative material

> **Not rules.** This file is the codex's companion, not its equal. Nothing here is binding. When a
> method and the codex disagree, the codex wins. When a method and a repository's own
> conventions disagree, the repository wins. Use these only where both are silent.

## Why this exists

The codex says **what must be true**. It does not say **how to make it true in a repository you have
never seen**. That is what this is: default moves, written as preferences, safe to ignore.

**This file names no technology.** No library, framework, language, vendor or team appears here. If a
default ever implies one, it is wrong — delete it. The repository is the authority on its own craft, and a
fallback that gets used when it should not be is a defect in this file, not in the repository.

---

## Prefer the repository to your habits

*Serves precepts 2 and 10.*

- Prefer the **nearest** code to your change over the style guide. The file you are editing is the
  convention.
- Prefer the **test suite** over the README. Tests are what the repository actually promises.
- Prefer the **last few changes** to that area over the documentation of it.
- Prefer the repository's **existing word** over a synonym you would introduce.
- Prefer the **build and test command the repository already documents** over the one you would choose.

## Prefer a decision to a description

*Serves precept 3.*

- Prefer a term the repository already **defines** — a type, an enum, a schema, a glossary entry — over the
  same word in prose.
- When two definitions disagree, prefer **raising the conflict** over picking a winner.
- Prefer the word the **domain owner** uses over the one in the ticket, and ask which is authoritative.

## Prefer one honest signal to several convenient ones

*Serves precepts 5 and 6.*

- Prefer **one check per criterion**, named after the criterion.
- Prefer the check that fails for the **reason the criterion names** over one that happens to fail.
- Prefer **proving it red first**. A check you never saw fail is a check you do not have.
- Prefer **declaring** a criterion you cannot check over writing a check that cannot fail.
- Prefer the repository's **existing** test layer over a new one.

## Prefer the smaller change

*Serves precept 9.*

- Prefer **extending an existing seam** over adding an abstraction.
- Prefer **two small changes** over one large one.
- Prefer leaving an out-of-scope problem **reported** over fixed.
- When unsure whether something is in scope, prefer to **ask** — it is out of scope until someone says
  otherwise.

## Prefer the neighbour's voice to your own

*Serves precept 10.*

- Prefer **no comment** to one that restates the code.
- Prefer the **error-handling idiom** of the file over a better one you know.
- Prefer the **naming pattern** already in the module over a more precise name.
- Prefer **deleting** a comment you cannot justify over keeping it.
- Prefer **nothing a reader could attribute to a machine**: no filler, no ceremony, no summary of the diff.

## Prefer a specific question to a broad one

*Serves precepts 1, 2 and 11.*

- Prefer **one fact, with the options you see**, over "is this okay?".
- Prefer stating **what you checked** and what you could not, over asking blind.
- Prefer **batching** the questions into one round over dripping them.
- Prefer a question **before** the work over a discovery after it.

## Prefer naming the risk to hiding it

*Serves precept 8.*

- Prefer the **change description** — the place the reviewer reads — over a comment in the code for
  residue.
- Prefer **naming the owner** over "someone should".
- Prefer **declaring the whole of it** over declaring only the comfortable part.

## Prefer evidence to assurance

*Serves precepts 4 and 7.*

- Prefer a **command and its output** over "verified".
- Prefer showing the **failing** run as well as the passing one.
- Prefer **listing every criterion and its signal** over summarising that it works.
- Prefer **rollback and observability stated** over implied.

---

## When a method and the repository disagree

The repository wins. You do not need to ask, and you do not need to note it.
