# Run Elvaris in QEMU from PowerShell

## Prerequisites
- QEMU installed at `C:\Program Files\qemu\qemu-system-x86_64.exe`
- OVMF firmware in `ovmf-x86_64\OVMF_CODE.fd` and `ovmf-x86_64\OVMF_VARS.fd`
- UEFI image built

## Build
```powershell
$env:PATH = "C:\Users\josep\.cargo\bin;$env:PATH"
cd C:\Users\josep\Desktop\Elvaris
cargo build -p os
```

## Find latest image
```powershell
Get-ChildItem -Path .\target -Recurse -Filter "uefi.img" | Sort-Object LastWriteTime -Descending | Select-Object -First 1 FullName
```

## Run
Replace <HASH> with the hash from above or use the latest found path.
```powershell
& "C:\Program Files\qemu\qemu-system-x86_64.exe" `
  -m 256M `
  -drive if=pflash,format=raw,readonly=on,file="C:\Users\josep\Desktop\Elvaris\ovmf-x86_64\OVMF_CODE.fd" `
  -drive if=pflash,format=raw,file="C:\Users\josep\Desktop\Elvaris\ovmf-x86_64\OVMF_VARS.fd" `
  -drive format=raw,file="C:\Users\josep\Desktop\Elvaris\target\x86_64-unknown-none\debug\build\os\<HASH>\out\uefi.img" `
  -serial stdio `
  -monitor none `
  -display none `
  -no-reboot
```

## Notes
- Press Ctrl+C to exit
- If heap panic, try `-m 512M`
