# EffectCraft for Android (phone layout)

EffectCraft's web build, with the phone layout (`crates/ui-egui/src/mobile.rs`), in an Android
WebView (Capacitor). Same engine, same panels, same project files; a different shell.

## APK बनाना (GitHub से, सबसे आसान)

1. यह पूरा प्रोजेक्ट अपने GitHub repository में push करें।
2. GitHub पर **Actions → Android APK → Run workflow** दबाएँ। (पहली बार में 30-60 मिनट लगते हैं।)
3. बन जाने पर उसी run के नीचे **Artifacts → EffectCraft-android-apk** डाउनलोड करें और ZIP खोलें।
4. APK को फ़ोन में भेजें, खोलें, और "Install unknown apps" की अनुमति दें।

## Build it yourself

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
# Node 20+, JDK 21, Android SDK (ANDROID_HOME) installed
bash packaging/android/build-apk.sh      # -> packaging/android/out/EffectCraft-<version>-debug.apk
```

## फ़ोन UI

- ऊपर: ☰ menu (File, Edit, Composition, Layer, Effect, Animation, View, Window, Help: सारे menus), project का नाम, Undo, Redo, viewer full screen, Play/Pause.
- बीच में: Composition viewer। नीचे: चुना हुआ panel। दोनों के बीच की पट्टी खींचकर size बदलें।
- सबसे नीचे: Project, Timeline, Controls (Effect Controls), Effects, More (बाकी सारे panels)।
- फ़ोन landscape में हो तो viewer बाएँ और panel दाएँ दिखता है।
- कोई बड़ी screen (tablet / PC) पर यही layout देखना हो तो ☰ → "Desktop layout" से वापस जाया जा सकता है; छोटी screen पर "Phone layout" बटन वापस ले आता है।

## Known limits

- First start loads a ~58 MB WebAssembly module (a few seconds). Projects, media and settings
  live in the app's private storage.
- Needs Android 8+ with a current Android System WebView (WebGPU on Android 12+; WebGL2 otherwise).
- Saving/rendering writes the file to `Documents/EffectCraft/` and opens the share sheet.
- Single-threaded wasm: heavy effects and long renders are slower than the desktop app.
- The debug APK is signed with a debug key. For the Play Store, sign a release build yourself.
