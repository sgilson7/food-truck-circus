# Tone

The rules every reader-visible string follows. Adopted as given from the Builder template (`template/TONE.md`) in October 2026, for the strings added under the privacy kit. `crates/core/tests/copy.rs` checks the word lists below and rule 2 on the strings whose key starts with `blocked_`; the older strings in `data/` predate this file and are not yet held to it (SECOND-ORDER). A reviewer checks the rest.

## Rules

1. **Lead with the point.** The first words of a string say what the reader needs: the result, the action, the reason.
2. **Plain words, short sentences.** Under 25 words a sentence. One idea a sentence.
3. **One name for each thing.** A term means what `GLOSSARY.md` says and is not renamed afterwards. The words on an entry's `Not:` line are never used for it.
4. **Numbers come from the core.** A count or limit a rule decides is filled in by the core, never typed into a string.
5. **A refusal names what is in the way and the next step.** No apology, no "something went wrong".
6. **No universals the tool cannot back.** *Every*, *always*, *never*, *only* and *nothing* state a rule of the tool, and a test shows it.
7. **Colour is never the only cue.** No string names a thing by its colour alone.
8. **No hype and no stock phrasing.** The lists below are the ones a test can check.

## Word lists for the lint

<!-- lint:start -->
Stock words:

- delve
- tapestry
- crucially
- robust
- leverage
- underscore
- landscape
- realm
- deep dive
- key takeaway
- seamless
- game-changer
- elevate
- empower

Announcing a point:

- it is worth noting
- it's worth noting
- it is important to

Contrast formulas:

- not only
- not just
- isn't just
- more than just

Filler:

- simply
- really
- actually

Apology and vague failure (rule 5):

- sorry
- something went wrong
- oops

Exclamation:

- !
<!-- lint:end -->
