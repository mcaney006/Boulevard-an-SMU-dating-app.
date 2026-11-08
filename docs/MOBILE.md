# Mobile apps

The plan is native UIs with the rules in Rust, shared through UniFFI:

- `boulevard-core` is compiled for `aarch64-apple-ios`, `aarch64-apple-ios-sim`, and
  `aarch64-linux-android` with the `ffi` feature, and `uniffi-bindgen` generates Swift and Kotlin
  bindings from the proc-macro annotations in `crates/boulevard-core/src/ffi.rs`.
- The apps call `check_email`, `validate_profile_input`, `score_candidate`, `catalog`, and
  `stand_ins` from the core, and talk to the API in `docs/API.md` over HTTPS and WebSocket.
- Both apps keep the same screens as the prototype: Deck, Likes, Chats, You, plus onboarding and the
  Plus panel. Every card shows the score reasons; the client never computes a different order than
  the server because both call the same function.

What exists today: the core compiles with the `ffi` feature (`cargo build -p boulevard-core
--features ffi`) and `scripts/bindings.sh` generates the Swift and Kotlin sources. The app shells are
the next piece of work; the API is complete enough to build them against now.
