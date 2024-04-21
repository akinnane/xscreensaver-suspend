# xscreensaver-suspend

Run from you WDM init scripts after xscreensaver. Uses `systemctl suspend` to trigger suspension. 

Will suspend at the same time XScreenSaver triggers DPMS off.

`touch ~/.no_suspend` to block suspending for 8 hours.
