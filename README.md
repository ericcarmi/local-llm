LLM server setup with mistralrs, client sends query and server streams response back, client uses enigo to write text directly into whatever editor you want (seems slow with vscode, better with terminal editors)

server is currently set up to run on Windows (cuz that's where my 4090 is) -- client has issues on Windows because of string parsing and enigo on windows seems to miss some messages, and the editor formatting makes a significant difference

client works well on mac -- copy query to clipboard and use shift + ctrl + alt to send query (device_query isn't recognizing left vs. right shift/alt)

https://github.com/user-attachments/assets/c3ad2cb0-b3f6-4f85-b2e5-e4bb6ddf838b

