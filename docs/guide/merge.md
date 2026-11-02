# Merge to HDR

A bracket shot on a tripod, the same picture at several exposures, can
be merged into one frame that holds the shadows of the brightest frame
and the highlights of the darkest.

- Select the bracket's frames, two or more, of one camera and one
  size, and choose **Merge to HDR** on the frame menu. The status line
  says each frame as it is read; a 45-megapixel frame takes a few
  seconds.
- The merge is a DNG beside the first frame, named after it:
  `IMG_0001-HDR.dng`, or `IMG_0001-HDR-1.dng` when that name is taken.
  When the frames share one folder and you are not in a collection, it
  goes on top of a stack with them.
- The file is a compact float DNG: about 170 MB for three 45-megapixel
  frames.
- It opens looking as the middle exposure did, with that frame's edit
  (its exposure left at zero), and the highlights the middle frame
  lost are there to bring down with **Highlights** or the exposure.
  **Highlights** at −100 brings the brightest of them just under
  white.
- The frames should not move between shots. A frame that moved a
  little is put back in place, and the log says so.
- Asking for a second merge while one runs starts it when the first is
  done, and the status line says so.
- The file is never seen half-written: it is written under a hidden
  name and renamed when done. If a merge is cut short, its hidden part
  file is removed the next time you merge to that name.

From the command line: `greycard merge hdr IMG_0001.CR3 IMG_0002.CR3
IMG_0003.CR3`, with `--reference` to pick the frame the others are put
on (counting from 0), `--out` to name the file, and `--on-exists` for
when that name is taken.
