
#import "/contrib/templates/std-tests/preset.typ": *
#show: test-page

// Test fence confusion.
$ |x + |y| + z/a| \
  |x + lr(|y|) + z/a| $
