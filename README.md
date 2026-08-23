# ytdlprustygui-tauri
a GUI front end for yt-dlp with tauri as the front end.

can download video and merge with the chosen audio file or download audio only as mp3.

also works with playlists.

Can now set default language preference.

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
cd ytdlprustygui-tauri
```

```
cargo tauri build
```

app will then be found in ytdlprustygui-tauri/src-tauri/target/release/bundle/

<img width="1012" height="812" alt="1 initial scren" src="https://github.com/user-attachments/assets/e5d0b97d-62ba-4c91-b776-6f4450e819aa" />

<img width="1012" height="812" alt="2 fetched formats" src="https://github.com/user-attachments/assets/c8097e87-eb49-4f8d-b711-c8b8e417463e" />

<img width="968" height="768" alt="3 downloading" src="https://github.com/user-attachments/assets/e92017f2-06a1-4387-8887-f8e2a3c0420a" />

<img width="968" height="768" alt="4 download complete" src="https://github.com/user-attachments/assets/11b4ec1b-b087-4c8f-94fd-4f7bbc1d677b" />

