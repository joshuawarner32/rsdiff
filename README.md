# RsDiff

At it's core, RsDiff is a re-implementation of BSDIFF, from Colin Percival.  As its name suggests, it's written in Rust, because that's the hip thing to do, right?

One of the goals of RsDiff is to tease apart all of the primitives of BSDIFF, to make them hackable and remixable.  For instance, it's easy with RsDiff to decide that bzip2 isn't really for you, and that you'd really rather use zstd as a compression backend.  Totally doable!

A more advanced transformation would be to play around with different command stream encodings.  Are there savings to be had by cutting the maximum offset size in half?  Find out!

