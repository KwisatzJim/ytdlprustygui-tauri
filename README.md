# ytdlprustygui-tauri
a GUI front end for yt-dlp with tauri as the front end.

can download video and merge with the chosen audio file or download audio only as mp3.

also works with playlists.

### To run it:

install rust

(official method from rust-lang.org)
```
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

install tauri-cli
```
cargo install tauri-cli --version "^2"
```

Clone this repository and build the app:

```
git clone https://github.com/KwisatzJim/ytdlprustygui
```

```
cd ytdlprustygui
```

<img width="1012" height="812" alt="Screenshot 2026-07-19 at 2 42 19 PM" src="https://github.com/user-attachments/assets/905ffcbf-99e9-47a4-9729-8a0ba60ca975" />


```
cargo tauri build
```

app will then be found in ytdlprustygui-tauri/src-tauri/target/release/bundle/
