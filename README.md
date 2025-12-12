SMT: Devil Summoner Disection Tools

These tools make it easy to split up and convert some of the .bin files included in SMT:Devil Summoner for the PSP and SMT:DS Soul Hackers for the PS1.

Included utils:
- gim2png - for SMT:DS PSP, converts PSP GIM files to PNG; this is not a universal tool, only really written for this use case.
- binextract - takes a SMT:DS PSP .bin archive file and extracts all the items in the file to seperate files, trying to match headers for filetypes and renaming accordingly. By default checks for the last entry being the string 'PSPCHECK' as per the game logic as an validity check.
- binsplit - for SMT:DS PSP, some .bin files are of a slightly different format (usually the *all.bin files), and these contain multiple files as well. Some of the extracted files are themselves .bin archives that can be further split by the other tool.
- imgsplit - split the PSXCD.IMG file in SMT:DS Soul Hackers on the PS1.
