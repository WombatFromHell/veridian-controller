# About this project

This program is intended to be a more polished attempt at implementing many of
the common Wayland-compatible Nvidia fan controller scripts available out there
in Rust with support for both Xorg/X11 and Wayland. Pascal and newer GPUs are
supported.

## Setup & Usage Instructions

**_Security disclaimer_**: due to design limitations in how Nvidia have implemented
fan-controls via the nvidia-settings interface a `sudoers.d` file (such as the one
included with this project) specifically for the `nvidia-settings` binary is
required in order to avoid password prompts when changing fan speeds using
automation. **This project takes absolutely no responsibility for any damage or
other such negative implications caused by the use of this or any other
open-source program.**

---

Now, with that out of the way, you can find release binaries as AppImage files
under the Actions section on the GitHub project page.

Before using `veridian-controller` you'll probably want to setup your environment
to use it with the least amount of friction:

- Make a sudoers.d file under `/etc/sudoers.d/` like so:

  ```bash
  sudo touch /etc/sudoers.d/99-nvidia-settings && \
  sudo chmod 0440 /etc/sudoers.d/99-nvidia-settings && \
  sudoedit /etc/sudoers.d/99-nvidia-settings
  ```

  - You'll want the content to be something like the following:
    - `yourusernamehere ALL=(ALL) NOPASSWD:/usr/bin/nvidia-settings`

- Customize the `veridian-controller.toml` config file under `~/.config/veridian-controller.toml`:

```toml
# represents temperature thresholds in celsius (must be monotonically increasing)
temp_thresholds = [40, 50, 60, 78, 84]
# represents target fan speed when crossing the matching temp threshold (must be monotonically increasing)
fan_speeds =      [46, 55, 62, 80, 100]
# the lowest fan speed that registers RPMs on the GPU fans
fan_speed_floor = 46
# this will either be 80 or 100 depending on what gen GPU you have
fan_speed_ceiling = 100
# the sampling window for averaging is comprised of X samples every Y seconds
sampling_window_size = 10
# the insensitivity boundary to speed/temperature changes
hysteresis = 3
# how frequently to poll the GPU for data
global_delay = 2
# how infrequently to send fan speed adjustments
fan_dwell_time = 10
# special mode that tries to smoothly adjust between the current speed and the target speed
smooth_mode = true
# increase incr_weight for less responsiveness when temperatures are increasing
smooth_mode_incr_weight = 1.0
# increase decr_weight for less responsiveness when temperatures are decreasing
smooth_mode_decr_weight = 4.0
# max amount of fan speed change per smooth mode adjustment period
smooth_mode_max_fan_step = 5
```

- A user-level systemd service file is included in the project directory as an
  example to customize for your convenience

- For example, one common installation environment (as a user-level systemd service that runs when the user logs in):

  ```bash
  mkdir -p ~/.local/bin ~/.config/systemd/user && \
  cp -f veridian-controller.AppImage ~/.local/bin/veridian-controller.AppImage && \
  ln -sf ~/.local/bin/veridian-controller.AppImage ~/.local/bin/veridian-controller && \
  cp -f veridian-controller.service ~/.config/systemd/user && \
  systemctl --user daemon-reload && systemctl --user enable --now veridian-controller
  ```
