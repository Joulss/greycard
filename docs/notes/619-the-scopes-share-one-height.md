# §619. The scopes share one height (2026-11-09)

The scope selector and Selection toggle sat below the plot. Switching
between RGB, Wave and Vector also changed the plot's height, and
Vector hid the camera and exposure lines to make room for its circle.
The panel below moved as the scope changed.

## The decision

The three scopes now use one 200 px container. The selector and
Selection toggle sit in its top row; the plot takes the remaining
height, and Vector is centered in it. Camera and exposure information
remain visible while switching scopes and fold away only in Cull mode.
The scope box loses its border, in keeping with the other panel
surfaces. Its selection and clipping controls retain their behavior.
