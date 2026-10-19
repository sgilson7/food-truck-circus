# Glossary

What each word in this tool's strings means. Started in October 2026 with the words of the Circus Math Lab's blocked-device messages (SECOND-ORDER K3); the game's older words are not yet entered. Write each entry as a `### Term` heading, one or two plain sentences, then an optional `Not:` line of words that must not be used for it. `crates/core/tests/copy.rs` checks the strings whose key starts with `blocked_` against the `Not:` words.

### JavaScript JIT

The browser setting, as Chrome names it, whose policy a school sets to block fast-compiled code. Blocking it can also block WebAssembly.

Not: Java

### WebAssembly

The compiled form of the lab's rules, which the browser runs. Without it the lab cannot start.

Not: plugin, extension, download, app

### WebGL

The browser's 3D drawing, which draws the stage and the performer. Chrome's policy for it is called the 3D APIs setting.

Not: graphics card, GPU, plugin
