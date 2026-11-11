# §637. English in the Language picker

*2026-11-11*

The Language row used to offer System and the installed translation
catalogs. On a system set to French, System and Français both chose
French, leaving no choice in the window to return to its English source
strings.

The row now offers System, English, and then the installed catalogs.
English is stored as `en` and selects the source strings regardless of
the system language. System remains the default and follows the system
language; a saved tag with no matching catalog also falls back to it.
As before, a change in this row takes effect on the next start.
