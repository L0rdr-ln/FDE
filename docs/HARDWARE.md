# Running FDE on the screen itself

`fde` started inside another desktop opens as a window there (nested). On a console it takes over
the screen and the input devices: that is how it runs as your desktop.

## Before the first try

```
~/fde/target/release/fde --check
```

It looks at the GPU, the monitors, the seat, OpenGL and the programs FDE uses, changes nothing,
and marks problems with `!!` and what to do about them. Send its output along when something does
not work.

In a **VMware** VM: switch on *Accelerate 3D graphics* in the VM's display settings and install
`open-vm-tools-desktop`. Without 3D, FDE still runs (drawn by the CPU, in fast mode), but slowly.

## Trying it

1. Press **Ctrl+Alt+F3** to get a text console, and log in.
2. Run `~/start-fde` (or `~/fde/target/release/fde drm`).
3. FDE takes the screen. **Alt+Esc** ends it and gives you the console back; **Ctrl+Alt+F1**
   (or F2) goes back to your normal desktop while FDE keeps running on its console, and
   Ctrl+Alt+F3 comes back to it.

What FDE printed is in `~/fde-last.log`.

## When it does not start

| message | what to do |
|---|---|
| `libseat: ... (is seatd or logind running ...)` | start it from a console you logged in on (not from a terminal inside a desktop, not with `su`); or `sudo systemctl enable --now seatd` and `sudo usermod -aG _seatd $USER`, then log in again |
| `no GPU found for this seat` | the VM has no display adapter with a driver: `fde --check` shows what is there |
| `EGL ... no OpenGL ES on this GPU` | switch on 3D acceleration (VMware: *Accelerate 3D graphics*), or start with `LIBGL_ALWAYS_SOFTWARE=1 ~/start-fde` |
| `no monitor is connected` | the VM's display is off or has no size: open the VM's console window |
| black screen after Ctrl+Alt+F1 and back | should not happen any more; if it does, send `~/fde-last.log` |
