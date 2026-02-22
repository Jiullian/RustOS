# RustOS
## Requirements  

### Install dependencies and configure the environment  
- Grant execute permissions to the script:  
  ```sh
  sudo chmod +x setup.sh
  ```
- Run it:  
  ```sh
  ./setup.sh
  ```
- It might trigger an error during `rust install` if Rust is already installed, but the process will continue normally.  

### Build the project  
```sh
cargo bootimage
qemu-system-x86_64 -drive format=raw,file=target/x86_64/debug/bootimage-RustOS.bin
```

### Or  
Simply run:  
```sh
./run.sh
```
