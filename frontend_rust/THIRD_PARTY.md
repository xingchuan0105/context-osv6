# UI source attribution

The Button and ChatComposer components adapt the button variants and input/footer
composition from [Rust/UI](https://github.com/rust-ui/ui), commit
`0c6b79e04a3ddc5fb9997e8cd9d9c8a6d9cbba9b`:

- `app_crates/registry/src/ui/button.rs`
- `app_crates/registry/src/ui/input_prompt.rs`

Local adaptations use Context-OS semantic CSS instead of Tailwind/macros, retain
native form submission and the existing composer resize behavior, add IME guards,
and connect existing conversation/attachment state through callbacks and signals.
The upstream app and its dependencies are not bundled.

## MIT License

Copyright (c) 2026 Max Wells

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
