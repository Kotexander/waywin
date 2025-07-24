# waywin - wayland and windows windowing library
many parts of waywin is based of [winit](https://github.com/rust-windowing/winit)

i personally only tested kde plasma, hyprland, and windows 11.
im pretty confident waywin won't work well in gnome (no CSD).

## why?
because i wanted to make windowing library

## ⚠️ still work in progress ⚠️
both windows and wayland are work in progress
## todos
### general
- fullscreen
### windows
- WIP
### wayland
- multi seat awareness (currently assumes only one seat with no checks)
- somehow integrate frame callback throttling, while allowing game loop updates
running as fast as wanted, while also staying consistent with windows
- touch support
- client side decorations
