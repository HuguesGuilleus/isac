# ISAC
Download and upload directory with SSH.

## Build
`cargo build --release`. You can alse use Github release to get binary.

## Usage

### Init
Create a directory and go inside then init it.
```bash
mkdir save
cd save
isac init
```
Add the public key into `~/.ssh/authorized_keys` of our servers and add the server list into the `list` file.

### Download
```bash
isac downlaod
```

### Uplaod
Isac don't overwrite old file.
```bash
isac upload
```
