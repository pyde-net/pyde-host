# Makefile.pyde — include this from your contract's Makefile to pick
# up the Pyde host header search path.
#
# Usage:
#
#     include path/to/pyde-host/c/Makefile.pyde
#     CFLAGS += $(PYDE_HOST_INCLUDE)
#
# Then in your source:
#
#     #include <pyde/host.h>
#
# The header itself is standalone — this fragment just spares you from
# hard-coding the include path in every contract Makefile.

PYDE_HOST_INCLUDE := -I$(dir $(lastword $(MAKEFILE_LIST)))include
