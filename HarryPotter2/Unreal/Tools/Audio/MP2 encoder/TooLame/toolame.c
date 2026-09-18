#include <stdlib.h>
#include "common.h"
#include "encoder.h"
#include "musicin.h"
#include "options.h"

void parse_input_file (char *, layer * info, unsigned long *num_samples);
void global_init (void);
void proginfo (void);
void short_usage (void);
int available_bits (layer * info, options * glopts);

#include <assert.h>

/* Global variable definitions for "musicin.c" */

FILE *musicin;
Bit_stream_struc bs;
char *programName;
char toolameversion[10] = "0.2i";

void
global_init (void)
{
  glopts.usepsy = TRUE;
  glopts.usepadbit = TRUE;
  glopts.quickmode = FALSE;
  glopts.quickcount = 10;
  glopts.downmix = FALSE;
  glopts.byteswap = FALSE;
  glopts.channelswap = FALSE;
#ifdef VBR
  glopts.vbr = FALSE;
  glopts.vbrlevel = 0;
#endif
}

/************************************************************************
*
* main
*
* PURPOSE:  MPEG II Encoder with
* psychoacoustic models 1 (MUSICAM) and 2 (AT&T)
*
* SEMANTICS:  One overlapping frame of audio of up to 2 channels are
* processed at a time in the following order:
* (associated routines are in parentheses)
*
* 1.  Filter sliding window of data to get 32 subband
* samples per channel.
* (window_subband,filter_subband)
*
* 2.  If joint stereo mode, combine left and right channels
* for subbands above #jsbound#.
* (combine_LR)
*
* 3.  Calculate scalefactors for the frame, and 
* also calculate scalefactor select information.
* (*_scale_factor_calc)
*
* 4.  Calculate psychoacoustic masking levels using selected
* psychoacoustic model.
* (psycho_i, psycho_ii)
*
* 5.  Perform iterative bit allocation for subbands with low
* mask_to_noise ratios using masking levels from step 4.
* (*_main_bit_allocation)
*
* 6.  If error protection flag is active, add redundancy for
* error protection.
* (*_CRC_calc)
*
* 7.  Pack bit allocation, scalefactors, and scalefactor select
* information onto bitstream.
* (*_encode_bit_alloc,*_encode_scale,transmission_pattern)
*
* 8.  Quantize subbands and pack them into bitstream
* (*_subband_quantization, *_sample_encoding)
*
************************************************************************/

int frameNum = 0;

int
main (int argc, char **argv)
{
  typedef double SBS[2][3][SCALE_BLOCK][SBLIMIT];
  SBS FAR *sb_sample;
  typedef double JSBS[3][SCALE_BLOCK][SBLIMIT];
  JSBS FAR *j_sample;
  typedef double IN[2][HAN_SIZE];
  IN FAR *win_que;
  typedef unsigned int SUB[2][3][SCALE_BLOCK][SBLIMIT];
  SUB FAR *subband;

  frame_params fr_ps;
  layer info;
  char original_file_name[MAX_NAME_SIZE];
  char encoded_file_name[MAX_NAME_SIZE];
  short FAR **win_buf;
  static short FAR buffer[2][1152];
  static unsigned int bit_alloc[2][SBLIMIT], scfsi[2][SBLIMIT];
  static unsigned int scalar[2][3][SBLIMIT], j_scale[3][SBLIMIT];
  static double FAR ltmin[2][SBLIMIT], lgmin[2][SBLIMIT], max_sc[2][SBLIMIT];
  FLOAT snr32[32];
  short sam[2][1344];		/* was [1056]; */
  int model, stereo, error_protection;
  static unsigned int crc;
  int i, j, k, adb;
  unsigned long frameBits, sentBits = 0;
  unsigned long num_samples;
  int lg_frame;
  int bytes_remaining;

  /* a bunch of SNR values I sort of made up  MFC 1 oct 99 */
  static FLOAT snrdef[2][32] = {
    {30, 17, 16, 10, 3, 12, 8, 2.5,
     5, 5, 6, 6, 5, 6, 10, 6,
     -4, -10, -21, -30, -42, -55, -68, -75,
     -75, -75, -75, -75, -91, -107, -110, -108},
    {30, 17, 16, 10, 3, 12, 8, 2.5,
     5, 5, 6, 6, 5, 6, 10, 6,
     -4, -10, -21, -30, -42, -55, -68, -75,
     -75, -75, -75, -75, -91, -107, -110, -108}
  };
  static int psycount = 0;
  extern int minimum;

  /* Most large variables are declared dynamically to ensure
     compatibility with smaller machines */

  sb_sample = (SBS FAR *) mem_alloc (sizeof (SBS), "sb_sample");
  j_sample = (JSBS FAR *) mem_alloc (sizeof (JSBS), "j_sample");
  win_que = (IN FAR *) mem_alloc (sizeof (IN), "Win_que");
  subband = (SUB FAR *) mem_alloc (sizeof (SUB), "subband");
  win_buf = (short FAR **) mem_alloc (sizeof (short *) * 2, "win_buf");

  /* clear buffers */
  memset ((char *) buffer, 0, sizeof (buffer));
  memset ((char *) bit_alloc, 0, sizeof (bit_alloc));
  memset ((char *) scalar, 0, sizeof (scalar));
  memset ((char *) j_scale, 0, sizeof (j_scale));
  memset ((char *) scfsi, 0, sizeof (scfsi));
  memset ((char *) ltmin, 0, sizeof (ltmin));
  memset ((char *) lgmin, 0, sizeof (lgmin));
  memset ((char *) max_sc, 0, sizeof (max_sc));
  memset ((char *) snr32, 0, sizeof (snr32));
  memset ((char *) sam, 0, sizeof (sam));

  global_init ();

  info.extension = 0;
  fr_ps.header = &info;
  fr_ps.tab_num = -1;		/* no table loaded */
  fr_ps.alloc = NULL;
  info.version = MPEG_AUDIO_ID;	/* Default: MPEG-1 */

  programName = argv[0];
  if (argc == 1)		/* no command-line args */
    short_usage ();
  else
    parse_args (argc, argv, &fr_ps, &model, &num_samples,
		original_file_name, encoded_file_name);
  print_config (&fr_ps, &model, original_file_name, encoded_file_name);

  hdr_to_frps (&fr_ps);
  stereo = fr_ps.stereo;
  error_protection = info.error_protection;

  while (get_audio (musicin, buffer, num_samples, stereo, &info) > 0)
    {
      if (++frameNum % 10 == 0)
	fprintf (stderr, "[%4u]\r", frameNum);
      fflush (stderr);
      win_buf[0] = &buffer[0][0];
      win_buf[1] = &buffer[1][0];

#ifdef VBR
      //info.bitrate_index=12;
#endif
      adb = available_bits (&info, &glopts);
      lg_frame = adb / 8;
      if (info.dab_extention)
	{
	  /* in 24 kHz we always have 4 bytes */
	  if (info.sampling_frequency == 1)
	    info.dab_extention = 4;
/* You must have one frame in memory if you are in DAB mode                 */
/* in conformity of the norme ETS 300 401 http://www.etsi.org               */
	  /* see bitstream.c            */
	  if (frameNum == 1)
	    minimum = lg_frame + MINIMUM;
	  adb -= info.dab_extention * 8 + 6 * 8;
	}

      for (i = 0; i < 3; i++)
	for (j = 0; j < SCALE_BLOCK; j++)
	  for (k = 0; k < stereo; k++)
	    {
	      window_subband (&win_buf[k], &(*win_que)[k][0], k);
	      filter_subband (&(*win_que)[k][0], &(*sb_sample)[k][i][j][0]);
	    }

      scale_factor_calc (*sb_sample, scalar, stereo, fr_ps.sblimit);
      pick_scale (scalar, &fr_ps, max_sc);
      if (fr_ps.actual_mode == MPG_MD_JOINT_STEREO)
	{
	  combine_LR (*sb_sample, *j_sample, fr_ps.sblimit);
	  scale_factor_calc (j_sample, &j_scale, 1, fr_ps.sblimit);
	}
      /* this way we calculate more mono than we need */
      /* but it is cheap */

      if (glopts.usepsy == FALSE)
	{			/* just make up the SNR values */
	  for (k = 0; k < stereo; k++)
	    {
	      for (i = 0; i < SBLIMIT; i++)
		ltmin[k][i] = snrdef[k][i];
	    }
	}
      else
	if ((glopts.quickmode == TRUE)
	    && (++psycount % glopts.quickcount != 0))
	{
	  /* just copy over the last values */
	  for (k = 0; k < stereo; k++)
	    {
	      for (i = 0; i < SBLIMIT; i++)
		ltmin[k][i] = snrdef[k][i];
	    }
	}
      else
	{
	  /* calculate the psy model */
	  if (model == 1)
	    psycho_i (buffer, max_sc, ltmin, &fr_ps);
	  else
	    {
	      /* model == 2 */
	      for (k = 0; k < stereo; k++)
		{
		  psycho_ii (&buffer[k][0], &sam[k][0], k, snr32,
			     (FLOAT) s_freq[info.version][info.
							  sampling_frequency]
			     * 1000);
		  for (i = 0; i < SBLIMIT; i++)
		    ltmin[k][i] = (double) snr32[i];
		}
	    }
	  if (glopts.quickmode == TRUE)
	    /* copy the ltmin values and reuse */
	    for (k = 0; k < stereo; k++)
	      {
		for (i = 0; i < SBLIMIT; i++)
		  snrdef[k][i] = ltmin[k][i];
	      }
	}


      transmission_pattern (scalar, scfsi, &fr_ps);
#ifdef VBR
      main_bit_allocation (ltmin, scfsi, bit_alloc, &adb, &fr_ps, &glopts);
#else
      main_bit_allocation (ltmin, scfsi, bit_alloc, &adb, &fr_ps);
#endif

      if (error_protection)
	CRC_calc (&fr_ps, bit_alloc, scfsi, &crc);

      encode_info (&fr_ps, &bs);

      if (error_protection)
	encode_CRC (crc, &bs);

      encode_bit_alloc (bit_alloc, &fr_ps, &bs);
      encode_scale (bit_alloc, scfsi, scalar, &fr_ps, &bs);
      subband_quantization (scalar, *sb_sample, j_scale,
			    *j_sample, bit_alloc, *subband, &fr_ps);
      sample_encoding (*subband, bit_alloc, &fr_ps, &bs);
      for (i = 0; i < adb; i++)
	put1bit (&bs, 0);
      if (info.dab_extention)
	{
/* reserved 4 bytes for X-PAD in DAB mode                              */
	  putbits (&bs, 0, 32);
	  for (i = info.dab_extention - 1; i >= 0; i--)
	    {
	      CRC_calcDAB (&fr_ps, bit_alloc, scfsi, scalar, &crc, i);
/* this crc is for the previous frame in DAB mode                      */
	      if (bs.buf_byte_idx + lg_frame < bs.buf_size)
		bs.buf[bs.buf_byte_idx + lg_frame] = crc;
/* reserved 2 bytes for F-PAD in DAB mode                              */
	      putbits (&bs, crc, 8);
	    }
	  putbits (&bs, 0, 16);
	}

      frameBits = sstell (&bs) - sentBits;
#ifdef VBR2
      fprintf (stderr, "Sent %ld bits = %ld slots plus %ld\n",
	       frameBits, frameBits / 8, frameBits % 8);
#else
      if (frameBits % 8)	/* a program failure */
	fprintf (stderr, "Sent %ld bits = %ld slots plus %ld\n",
		 frameBits, frameBits / 8, frameBits % 8);
#endif
      sentBits += frameBits;
    }

  close_bit_stream_w (&bs);

  fprintf (stderr,
	   "Avg slots/frame = %.3f; b/smp = %.2f; bitrate = %.3f kbps\n",
	   (FLOAT) sentBits / (frameNum * 8),
	   (FLOAT) sentBits / (frameNum * 1152),
	   (FLOAT) sentBits / (frameNum * 1152) *
	   s_freq[info.version][info.sampling_frequency]);

  if (fclose (musicin) != 0)
    {
      fprintf (stderr, "Could not close \"%s\".\n", original_file_name);
      exit (2);
    }

  fprintf (stderr, "\nDone\n");
  exit (0);
}

/************************************************************************
*
* print_config
*
* PURPOSE:  Prints the encoding parameters used
*
************************************************************************/

void
print_config (frame_params * fr_ps, int *psy, char *inPath, char *outPath)
{
  layer *info = fr_ps->header;

  fprintf (stderr, "--------------------------------------------\n");
#ifdef IOFIX
  fprintf (stderr, "Input File : '%s'   %.1f kHz\n",
	   (strcmp (inPath, "-") ? inPath : "stdin"),
	   s_freq[info->version][info->sampling_frequency]);
  fprintf (stderr, "Output File: '%s'\n",
	   (strcmp (outPath, "-") ? outPath : "stdout"));
#else
  fprintf (stderr, "Input File : '%s'   %.1f kHz\n", inPath,
	   s_freq[info->version][info->sampling_frequency]);
  fprintf (stderr, "Output File: '%s'\n", outPath);
#endif
  fprintf (stderr, "%d kbps ", bitrate[info->version][info->bitrate_index]);
  fprintf (stderr, "%s ", version_names[info->version]);
  if (info->mode != MPG_MD_JOINT_STEREO)
    fprintf (stderr, "Layer II %s Psycho model=%d  (Mode_Extension=%d)\n",
	     mode_names[info->mode], *psy, info->mode_ext);
  else
    fprintf (stderr, "Layer II %s Psy model %d \n",
	     mode_names[info->mode], *psy);

  fprintf (stderr, "[De-emph:%s\tCopyright:%s\tOriginal:%s\tCRC:%s]\n",
	   ((info->emphasis) ? "On" : "Off"),
	   ((info->copyright) ? "Yes" : "No"),
	   ((info->original) ? "Yes" : "No"),
	   ((info->error_protection) ? "On" : "Off"));

  fprintf (stderr, "[Padding:%s\tByte-swap:%s\tChanswap:%s\tDAB:%s]\n",
	   ((glopts.usepadbit) ? "Normal" : "Off"),
	   ((glopts.byteswap) ? "On" : "Off"),
	   ((glopts.channelswap) ? "On" : "Off"),
	   ((glopts.dab) ? "On" : "Off"));


#ifdef VBR
  if (glopts.vbr == TRUE)
    fprintf (stderr, "VBR %i\n", glopts.vbrlevel);
#endif
  fprintf (stderr, "--------------------------------------------\n");
}


/************************************************************************
*
* usage
*
* PURPOSE:  Writes command line syntax to the file specified by #stderr#
*
************************************************************************/

void
usage (void)			/* print syntax & exit */
{
  /* FIXME: maybe have an option to display better definitions of help codes, and
     long equivalents of the flags */
  fprintf (stdout, "\ntooLAME version %s (http://toolame.sourceforge.net)\n",
	   toolameversion);
  fprintf (stdout, "MPEG Audio Layer II encoder\n\n");
  fprintf (stdout, "usage: \n");
  fprintf (stdout, "\t%s [options] <input> <output>\n\n", programName);

  fprintf (stdout, "Options:\n");
  fprintf (stdout, "Input\n");
  fprintf (stdout, "\t-s sfrq  input smpl rate in kHz   (dflt %4.1f)\n",
	   DFLT_SFQ);
  fprintf (stdout, "\t-a       downmix from stereo to mono\n");
  fprintf (stdout, "\t-x       force byte-swapping of input\n");
  fprintf (stdout, "\t-g       swap channels of input file\n");
  fprintf (stdout, "Output\n");
  fprintf (stdout, "\t-m mode  channel mode : s/d/j/m   (dflt %4c)\n",
	   DFLT_MOD);
  fprintf (stdout, "\t-p psy   psychoacoustic model 1/2 (dflt %4u)\n",
	   DFLT_PSY);
  fprintf (stdout, "\t-b br    total bitrate in kbps    (dflt 192)\n");
#ifdef VBR
  fprintf (stdout, "\t-v lev   vbr mode\n");
#endif
  fprintf (stdout, "Operation\n");
  fprintf (stdout, "\t-f       fast mode (turns off psy model)\n");
  fprintf (stdout,
	   "\t-q num   quick mode. only calculate psy model every num frames\n");
  fprintf (stdout, "Misc\n");
  fprintf (stdout, "\t-d emp   de-emphasis n/5/c        (dflt %4c)\n",
	   DFLT_EMP);
  fprintf (stdout, "\t-c       mark as copyright\n");
  fprintf (stdout, "\t-o       mark as original\n");
  fprintf (stdout, "\t-e       add error protection\n");
  fprintf (stdout, "\t-r       force padding bit/frame off\n");
  fprintf (stdout, "\t-D       add DAB extentions\n");
  fprintf (stdout, "Files\n");
  fprintf (stdout,
	   "\tinput    input sound file. (WAV,AIFF,PCM or use '/dev/stdin')\n");
  fprintf (stdout, "\toutput   output bit stream of encoded audio\n");
  fprintf (stdout,
	   "\n\tAllowable bitrates for 16, 22.05 and 24kHz sample input\n");
  fprintf (stdout,
	   "\t8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160\n");
  fprintf (stdout,
	   "\n\tAllowable bitrates for 32, 44.1 and 48kHz sample input\n");
  fprintf (stdout,
	   "\t32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320\n");
  exit (1);
}

/*********************************************
 * void short_usage(void)
 ********************************************/
void
short_usage (void)
{
  /* print a bit of info about the program */
  fprintf (stderr, "tooLAME version %s\n (http://toolame.sourceforge.net)\n",
	   toolameversion);
  fprintf (stderr, "MPEG Audio Layer II encoder\n\n");
  fprintf (stderr, "USAGE: %s [options] <infile> [outfile]\n\n", programName);
  fprintf (stderr, "Try \"%s -h\" for more information.\n", programName);
  exit (0);
}

/*********************************************
 * void proginfo(void)
 ********************************************/
void
proginfo (void)
{
  /* print a bit of info about the program */
  fprintf (stderr,
	   "\ntooLAME version 0.2g (http://toolame.sourceforge.net)\n");
  fprintf (stderr, "MPEG Audio Layer II encoder\n\n");
}

/************************************************************************
*
* parse_args
*
* PURPOSE:  Sets encoding parameters to the specifications of the
* command line.  Default settings are used for parameters
* not specified in the command line.
*
* SEMANTICS:  The command line is parsed according to the following
* syntax:
*
* -m  is followed by the mode
* -p  is followed by the psychoacoustic model number
* -s  is followed by the sampling rate
* -b  is followed by the total bitrate, irrespective of the mode
* -d  is followed by the emphasis flag
* -c  is followed by the copyright/no_copyright flag
* -o  is followed by the original/not_original flag
* -e  is followed by the error_protection on/off flag
* -f  turns off psy model (fast mode)
* -q <i>  only calculate psy model every ith frame
* -a  downmix from stereo to mono 
* -r  turn off padding bits in frames.
* -x  force byte swapping of input
* -g  swap the channels on an input file
*
* If the input file is in AIFF format, the sampling frequency is read
* from the AIFF header.
*
* The input and output filenames are read into #inpath# and #outpath#.
*
************************************************************************/

void
parse_args (int argc,
	    char **argv,
	    frame_params * fr_ps,
	    int *psy,
	    unsigned long *num_samples,
	    char inPath[MAX_NAME_SIZE], char outPath[MAX_NAME_SIZE])
{
  FLOAT srate;
  int brate;
  layer *info = fr_ps->header;
  int err = 0, i = 0;
  long samplerate;

  /* preset defaults */
  inPath[0] = '\0';
  outPath[0] = '\0';
  info->lay = DFLT_LAY;
  switch (DFLT_MOD)
    {
    case 's':
      info->mode = MPG_MD_STEREO;
      info->mode_ext = 0;
      break;
    case 'd':
      info->mode = MPG_MD_DUAL_CHANNEL;
      info->mode_ext = 0;
      break;
      /* in j-stereo mode, no default info->mode_ext was defined, gave error..
         now  default = 2   added by MFC 14 Dec 1999.  */
    case 'j':
      info->mode = MPG_MD_JOINT_STEREO;
      info->mode_ext = 2;
      break;
    case 'm':
      info->mode = MPG_MD_MONO;
      info->mode_ext = 0;
      break;
    default:
      fprintf (stderr, "%s: Bad mode dflt %c\n", programName, DFLT_MOD);
      abort ();
    }
  *psy = DFLT_PSY;
  if ((info->sampling_frequency =
       SmpFrqIndex ((long) (1000 * DFLT_SFQ), &info->version)) < 0)
    {
      fprintf (stderr, "%s: bad sfrq default %.2f\n", programName, DFLT_SFQ);
      abort ();
    }
  info->bitrate_index = 14;
  brate = 0;
  switch (DFLT_EMP)
    {
    case 'n':
      info->emphasis = 0;
      break;
    case '5':
      info->emphasis = 1;
      break;
    case 'c':
      info->emphasis = 3;
      break;
    default:
      fprintf (stderr, "%s: Bad emph dflt %c\n", programName, DFLT_EMP);
      abort ();
    }
  info->copyright = 0;
  info->original = 0;
  info->error_protection = FALSE;
  info->dab_extention = 0;

  /* process args */
  while (++i < argc && err == 0)
    {
      char c, *token, *arg, *nextArg;
      int argUsed;

      token = argv[i];
      if (*token++ == '-')
	{
	  if (i + 1 < argc)
	    nextArg = argv[i + 1];
	  else
	    nextArg = "";
	  argUsed = 0;
#ifdef IOFIX
	  if (!*token)
	    {
	      /* The user wants to use stdin and/or stdout. */
	      if (inPath[0] == '\0')
		strncpy (inPath, argv[i], MAX_NAME_SIZE);
	      else if (outPath[0] == '\0')
		strncpy (outPath, argv[i], MAX_NAME_SIZE);
	    }
#endif
	  while ((c = *token++))
	    {
	      if (*token /* NumericQ(token) */ )
		arg = token;
	      else
		arg = nextArg;
	      switch (c)
		{
		case 'm':
		  argUsed = 1;
		  if (*arg == 's')
		    {
		      info->mode = MPG_MD_STEREO;
		      info->mode_ext = 0;
		    }
		  else if (*arg == 'd')
		    {
		      info->mode = MPG_MD_DUAL_CHANNEL;
		      info->mode_ext = 0;
		    }
		  else if (*arg == 'j')
		    {
		      info->mode = MPG_MD_JOINT_STEREO;
		    }
		  else if (*arg == 'm')
		    {
		      info->mode = MPG_MD_MONO;
		      info->mode_ext = 0;
		    }
		  else
		    {
		      fprintf (stderr, "%s: -m mode must be s/d/j/m not %s\n",
			       programName, arg);
		      err = 1;
		    }
		  break;
		case 'p':
		  *psy = atoi (arg);
		  argUsed = 1;
		  if (*psy < 1 || *psy > 2)
		    {
		      fprintf (stderr,
			       "%s: -p model must be 1 or 2, not %s\n",
			       programName, arg);
		      err = 1;
		    }
		  break;

		case 's':
		  argUsed = 1;
		  srate = atof (arg);
		  /* samplerate = rint( 1000.0 * srate ); $A  */
		  samplerate = (long) ((1000.0 * srate) + 0.5);
		  if ((info->sampling_frequency =
		       SmpFrqIndex ((long) samplerate, &info->version)) < 0)
		    err = 1;
		  break;

		case 'b':
		  argUsed = 1;
		  brate = atoi (arg);
		  break;
		case 'd':
		  argUsed = 1;
		  if (*arg == 'n')
		    info->emphasis = 0;
		  else if (*arg == '5')
		    info->emphasis = 1;
		  else if (*arg == 'c')
		    info->emphasis = 3;
		  else
		    {
		      fprintf (stderr, "%s: -d emp must be n/5/c not %s\n",
			       programName, arg);
		      err = 1;
		    }
		  break;
		case 'D':
		  info->dab_extention = 2;
		  glopts.dab = TRUE;
		  break;
		case 'c':
		  info->copyright = 1;
		  break;
		case 'o':
		  info->original = 1;
		  break;
		case 'e':
		  info->error_protection = TRUE;
		  break;
		case 'f':
		  glopts.usepsy = FALSE;
		  break;
		case 'r':
		  glopts.usepadbit = FALSE;
		  info->padding = 0;
		  break;
		case 'q':
		  argUsed = 1;
		  glopts.quickmode = TRUE;
		  glopts.usepsy = TRUE;
		  glopts.quickcount = atoi (arg);
		  if (glopts.quickcount == 0)
		    {
		      /* just don't use psy model */
		      glopts.usepsy = FALSE;
		      glopts.quickcount = FALSE;
		    }
		  break;
		case 'a':
		  glopts.downmix = TRUE;
		  info->mode = MPG_MD_MONO;
		  info->mode_ext = 0;
		  break;
		case 'x':
		  glopts.byteswap = TRUE;
		  break;
#ifdef VBR
		case 'v':
		  argUsed = 1;
		  glopts.vbr = TRUE;
		  glopts.vbrlevel = atoi (arg);
		  glopts.usepadbit = FALSE;	/* don't use padding for VBR */
		  info->padding = 0;
		  info->mode = MPG_MD_STEREO;	/* force stereo mode */
		  info->mode_ext = 0;
		  break;
#endif
		case 'h':
		  usage ();
		  break;
		case 'g':
		  glopts.channelswap = TRUE;
		  break;
		default:
		  fprintf (stderr, "%s: unrec option %c\n", programName, c);
		  err = 1;
		  break;
		}
	      if (argUsed)
		{
		  if (arg == token)
		    token = "";	/* no more from token */
		  else
		    ++i;	/* skip arg we used */
		  arg = "";
		  argUsed = 0;
		}
	    }
	}
      else
	{
	  if (inPath[0] == '\0')
	    strcpy (inPath, argv[i]);
	  else if (outPath[0] == '\0')
	    strcpy (outPath, argv[i]);
	  else
	    {
	      fprintf (stderr, "%s: excess arg %s\n", programName, argv[i]);
	      err = 1;
	    }
	}
    }

  /* check for a valid bitrate */
  if (brate == 0)
    brate = bitrate[info->version][10];

  if (info->dab_extention)
    {
      /* in 48 kHz */
      /* if the bit rate per channel is less then 56 kbit/s, we have 2 scf-crc */
      /* else we have 4 scf-crc */
      /* in 24 kHz, we have 4 scf-crc, see main loop */
      if (brate / (info->mode == MPG_MD_MONO ? 1 : 2) >= 56)
	info->dab_extention = 4;
    }


  if (err || inPath[0] == '\0')
    usage ();			/* If no infile defined, or err has occured, then call usage() */

  if (outPath[0] == '\0')
    {
      /* replace old extension with new one, 1992-08-19, 1995-06-12 shn */
      new_ext (inPath, DFLT_EXT, outPath);
    }
#ifdef IOFIX
  if (!strcmp (inPath, "-"))
    {
      musicin = stdin;		/* read from stdin */
      *num_samples = MAX_U_32_NUM;
    }
  else
    {
      if ((musicin = fopen (inPath, "rb")) == NULL)
	{
	  fprintf (stderr, "Could not find \"%s\".\n", inPath);
	  exit (1);
	}
      parse_input_file (inPath, info, num_samples);
    }

#else
  if ((musicin = fopen (inPath, "rb")) == NULL)
    {
      fprintf (stderr, "Could not find \"%s\".\n", inPath);
      exit (1);
    }
  parse_input_file (inPath, info, num_samples);
#endif

  if ((info->bitrate_index = BitrateIndex (brate, info->version)) < 0)
    err = 1;

  open_bit_stream_w (&bs, outPath, BUFFER_SIZE);

}

/************************************************************
*   parse_input_file()
*   Determine the type of sound file. (stdin, wav, aiff, raw pcm)
*   Determine Sampling Frequency
*             number of samples
*	      whether the new sample is stereo or mono. 
*
*   If file is coming from /dev/stdin assume it is raw PCM. (it's what I use. YMMV)
*
*  This is just a hacked together function. The aiff parsing comes from the ISO code.
*  The WAV code comes from Nick Burch
*  The ugly /dev/stdin hack comes from me.
*                                                  MFC Dec 99
**************************************************************/
void
parse_input_file (char inPath[MAX_NAME_SIZE], layer * info,
		  unsigned long *num_samples)
{

  IFF_AIFF pcm_aiff_data;
  long soundPosition;

  char wave_header_buffer[44];
  int wave_header_read = 0;
  int wave_header_stereo = -1;
  int wave_header_16bit = -1;
  long samplerate;
  unsigned long *pDataSize;
  unsigned short *pNumChannels;

  /*************************** STDIN ********************************/
  /* check if we're reading from stdin. Assume it's a raw PCM file. */
  /* Of course, you could be piping a WAV file into stdin. Not done in this code */
  /* this code is probably very dodgy and was written to suit my needs. MFC Dec 99 */
  if ((strcmp (inPath, "/dev/stdin") == 0))
    {
      fprintf (stderr, "Reading from stdin\n");
      fprintf (stderr, "Remember to set samplerate with '-s'.\n");
      *num_samples = MAX_U_32_NUM;	/* huge sound file */
      return;
    }

  /****************************  AIFF ********************************/
  if ((soundPosition = aiff_read_headers (musicin, &pcm_aiff_data)) != -1)
    {
      fprintf (stderr, ">>> Using Audio IFF sound file headers\n");
      aiff_check (inPath, &pcm_aiff_data, &info->version);
      if (fseek (musicin, soundPosition, SEEK_SET) != 0)
	{
	  fprintf (stderr, "Could not seek to PCM sound data in \"%s\".\n",
		   inPath);
	  exit (1);
	}
      fprintf (stderr, "Parsing AIFF audio file \n");
      info->sampling_frequency =
	SmpFrqIndex ((long) pcm_aiff_data.sampleRate, &info->version);
      fprintf (stderr, ">>> %f Hz sampling frequency selected\n",
	       pcm_aiff_data.sampleRate);

      /* Determine number of samples in sound file */
      *num_samples = pcm_aiff_data.numChannels *
	pcm_aiff_data.numSampleFrames;

      if (pcm_aiff_data.numChannels == 1)
	{
	  info->mode = MPG_MD_MONO;
	  info->mode_ext = 0;
	}
      return;
    }

  /**************************** WAVE *********************************/
  /*   Nick Burch <The_Leveller@newmail.net> */
  /*********************************/
  /* Wave File Headers:   (Dec)    */
  /* 8-11 = "WAVE"                 */
  /* 22 = Stereo / Mono            */
  /*       01 = mono, 02 = stereo  */
  /* 24 = Sampling Frequency       */
  /*       17 = 11.025 kHz         */
  /*     ? 22 = 16.000 kHz         */
  /*       34 = 22.050 kHz         */
  /*       50 = 32.000 kHz         */
  /*       68 = 44.100 kHz         */
  /*     -128 = 44.800 kHz         */
  /*     -110 =  5.500 kHz         */
  /*      124 = 22.254 kHz         */
  /* 32 = Data Rate                */
  /*       01 = x1 (8bit Mono)     */
  /*       02 = x2 (8bit Stereo or */
  /*                16bit Mono)    */
  /*       04 = x4 (16bit Stereo)  */
  /*********************************/

  fseek (musicin, 0, SEEK_SET);
  fread (wave_header_buffer, 1, 44, musicin);

  if (wave_header_buffer[8] == 'W' && wave_header_buffer[9] == 'A' &&
      wave_header_buffer[10] == 'V' && wave_header_buffer[11] == 'E')
    {
      fprintf (stderr, "Parsing Wave File Header\n");
      /* Wave File */
      wave_header_read = 1;
      switch ((long) wave_header_buffer[24])
	{
	case 68:
	  /* 44.1 kHz */
	  samplerate = 44100;
	  fprintf (stderr, ">>> 44.1 Hz sampling freq selected\n");
	  break;
	case -128:
	  /* 48.0 kHz */
	  samplerate = 48000;
	  fprintf (stderr, ">>> 48.0 Hz sampling freq selected\n");
	  break;
	case 50:
			   /* 32.0 kHz *//********TO BE CHECKED!!*******/
	  samplerate = 32000;
	  fprintf (stderr, ">>> 32.0 Hz sampling freq selected\n");
	  break;
	case 34:
	  /* 22.05 kHz */
	  fprintf (stderr, ">>> 22.05 kHz Sampling freq selected\n");
	  samplerate = 22050;
	  break;
	case 22:
			  /* 16.0 kHz *//*********TO BE CHECKED!!*********/
	  fprintf (stderr, ">>> 16.0 kHz Sampling freq selected\n");
	  samplerate = 16000;
	  break;
	default:
	  /* Unknown Unsupported Frequency */
	  fprintf (stderr, ">>> Unable to get samp freq from Wave Header\n");
	  fprintf (stderr, ">>> Default 44.1 kHz samp freq selected\n");
	  samplerate = 44100;
	}
      if ((info->sampling_frequency =
	   SmpFrqIndex ((long) samplerate, &info->version)) < 0)
	{
	  fprintf (stderr, "invalid sample rate\n");
	  exit (0);
	}

      if ((long) wave_header_buffer[22] == 1)
	{
	  fprintf (stderr, ">>> Input Wave File is Mono\n");
	  wave_header_stereo = 0;
	  info->mode = MPG_MD_MONO;
	  info->mode_ext = 0;
	}
      if ((long) wave_header_buffer[22] == 2)
	{
	  fprintf (stderr, ">>> Input Wave File is Stereo\n");
	  wave_header_stereo = 1;
	}
      if ((long) wave_header_buffer[32] == 1)
	{
	  fprintf (stderr, ">>> Input Wave File is 8 Bit\n");
	  wave_header_16bit = 0;
	  fprintf (stderr, "Input File must be 16 Bit! Please Re-sample");
	  exit (1);
	}
      if ((long) wave_header_buffer[32] == 2)
	{
	  if (wave_header_stereo == 1)
	    {
	      fprintf (stderr, ">>> Input Wave File is 8 Bit\n");
	      wave_header_16bit = 0;
	      fprintf (stderr, "Input File must be 16 Bit! Please Re-sample");
	      exit (1);
	    }
	  else
	    {
	      /* fprintf(stderr,  ">>> Input Wave File is 16 Bit\n" ); */
	      wave_header_16bit = 1;
	    }
	}
      if ((long) wave_header_buffer[32] == 4)
	{
	  /* fprintf(stderr,  ">>> Input Wave File is 16 Bit\n" ); */
	  wave_header_16bit = 1;
	}

    if (wave_header_buffer[34] != 16)
	{		
		fprintf (stderr, "We don't support other than 16-bit sounds! \"%s\".\n", inPath);
		exit (1);
	}

#if 1
	pDataSize = (unsigned long *)&wave_header_buffer[40];
	pNumChannels = (unsigned short *)&wave_header_buffer[22];

	*num_samples = *pDataSize / sizeof(short) / *pNumChannels;
#else
      *num_samples = MAX_U_32_NUM;
#endif

      if (fseek (musicin, 44, SEEK_SET) != 0)
	{			/* there's a way of calculating the size of the
				   wave header. i'll just jump 44 to start with */
	  fprintf (stderr, "Could not seek to PCM sound data in \"%s\".\n",
		   inPath);
	  exit (1);
	}
      return;
    }

  /*************************** PCM **************************/
  fprintf (stderr, "No header found. Assuming Raw PCM sound file\n");
  *num_samples = MAX_U_32_NUM;	/* huge sound file */
}



/************************************************************************
*
* aiff_check
*
* PURPOSE:  Checks AIFF header information to make sure it is valid.
*           Exits if not.
*
************************************************************************/

void
aiff_check (char *file_name, IFF_AIFF * pcm_aiff_data, int *version)
{
  if (pcm_aiff_data->sampleType != IFF_ID_SSND)
    {
      fprintf (stderr, "Sound data is not PCM in \"%s\".\n", file_name);
      exit (1);
    }

  if (SmpFrqIndex ((long) pcm_aiff_data->sampleRate, version) < 0)
    {
      fprintf (stderr, "in \"%s\".\n", file_name);
      exit (1);
    }

  if (pcm_aiff_data->sampleSize != sizeof (short) * BITS_IN_A_BYTE)
    {
      fprintf (stderr, "Sound data is not %d bits in \"%s\".\n",
	       sizeof (short) * BITS_IN_A_BYTE, file_name);
      exit (1);
    }

  if (pcm_aiff_data->numChannels != MONO &&
      pcm_aiff_data->numChannels != STEREO)
    {
      fprintf (stderr, "Sound data is not mono or stereo in \"%s\".\n",
	       file_name);
      exit (1);
    }

  if (pcm_aiff_data->blkAlgn.blockSize != 0)
    {
      fprintf (stderr, "Block size is not %d bytes in \"%s\".\n", 0,
	       file_name);
      exit (1);
    }

  if (pcm_aiff_data->blkAlgn.offset != 0)
    {
      fprintf (stderr, "Block offset is not %d bytes in \"%s\".\n", 0,
	       file_name);
      exit (1);
    }
}
