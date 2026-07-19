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

```
cargo tauri build
```

app will then be found in ytdlprustygui-tauri/src-tauri/target/release/bundle/
