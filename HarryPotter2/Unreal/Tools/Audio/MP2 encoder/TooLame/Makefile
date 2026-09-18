
CC = gcc

c_sources = \
	common.c \
	encode.c \
	formatBitstream.c \
	ieeefloat.c \
	toolame.c \
	portableio.c \
	psycho_I.c \
	psycho_II.c \
	fft.c \
	subband.c \
	get_audio.c \
	bitstream.c \
	mem.c \
	crc.c \
	tables.c \
	availbits.c

OBJ = $(c_sources:.c=.o)
DEP = $(c_sources:.c=.d)

PG = -pg
#PG = -fomit-frame-pointer
#OPTIM = -O9
OPTIM = -O -g -DMDEBx
#DMAL = 
DMAL = -DDMALLOC -DMEMFIX
DMALLOCLIB = -ldmalloc
REQUIRED = -DMFCMISC -DUNIX -DNDEBUG -DINLINE=inline $(DMAL)
ARCH = -march=pentium 
#Following TWEAKS are for gcc2.95.2 YMMV.
TWEAKS = -finline-functions -fexpensive-optimizations -ffast-math -malign-double \
	-mfancy-math-387 -funroll-loops -funroll-all-loops -pipe \
	-fschedule-insns2 -fno-strength-reduce
#EXTRATWEAKS = -DVBRDEBUG
WARNING = -Wall -Wstrict-prototypes -Wmissing-prototypes
WORKING = -DVBR -DIOFIX
CC_SWITCHES = $(OPTIM) $(REQUIRED) $(ARCH) $(PG) $(TWEAKS) $(EXTRATWEAKS) $(WARNING) $(WORKING)

PGM = toolame

LIBS =  -lm $(DMALLOCLIB)

#nick burch's OS/2 fix  gagravarr@SoftHome.net
UNAME = $(shell uname)
ifeq ($(UNAME),OS/2)
   SHELL=sh     
   PGM = toolame.exe
   LIBS =
endif

%.o: %.c 
	$(CC) $(CC_SWITCHES) -c $< -o $@

%.d: %.c
	$(SHELL) -ec '$(CC) -M $(CC_SWITCHES) $< | sed '\''s/$*.o/& $@/g'\'' > $@'


$(PGM):	$(OBJ) Makefile
	$(CC) $(PG) -o $(PGM) $(OBJ) $(LIBS)

clean:
	-rm $(OBJ) $(DEP)

megaclean:
	-rm $(OBJ) $(DEP) $(PGM) \#*\# *~

distclean:
	-rm $(OBJ) $(DEP) $(PGM) \#* *~ gmon.out gprof* core *shit* *.wav *.mp2 *.c.* *.mp2.* *.da *.h.* *.d *.mp3 *.pcm *.wav logfile

tags: TAGS

TAGS: ${c_sources}
	etags -T ${c_sources}

-include $(DEP)
