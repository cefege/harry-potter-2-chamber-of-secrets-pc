#include <stdlib.h>
#include "common.h"
#include "encoder.h"
#include "options.h"


/************************************************************************
*
* read_samples()
*
* PURPOSE:  reads the PCM samples from a file to the buffer
*
*  SEMANTICS:
* Reads #samples_read# number of shorts from #musicin# filepointer
* into #sample_buffer[]#.  Returns the number of samples read.
*
************************************************************************/

unsigned long
read_samples (FILE * musicin,
	      short sample_buffer[2304],
	      unsigned long num_samples, unsigned long frame_size)
{
  unsigned long samples_read;
  static unsigned long samples_to_read;
  static char init = TRUE;

  if (init)
    {
      samples_to_read = num_samples;
      init = FALSE;
    }
  if (samples_to_read >= frame_size)
    samples_read = frame_size;
  else
    samples_read = samples_to_read;
  if ((samples_read =
       fread (sample_buffer, sizeof (short), (int) samples_read,
	      musicin)) == 0)
    fprintf (stderr, "Hit end of audio data\n");
  /*
     Samples are big-endian. If this is a little-endian machine
     we must swap
   */
  if (NativeByteOrder == order_unknown)
    {
      NativeByteOrder = DetermineByteOrder ();
      if (NativeByteOrder == order_unknown)
	{
	  fprintf (stderr, "byte order not determined\n");
	  exit (1);
	}
    }
  if (NativeByteOrder != order_littleEndian || (glopts.byteswap == TRUE))
    SwapBytesInWords (sample_buffer, samples_read);

  samples_to_read -= samples_read;
  if (samples_read < frame_size && samples_read > 0)
    {
      fprintf (stderr,
	       "Insufficient PCM input for one frame - fillout with zeros\n");
      for (; samples_read < frame_size; sample_buffer[samples_read++] = 0);
      samples_to_read = 0;
    }
  return (samples_read);
}

/************************************************************************
*
* get_audio()
*
* PURPOSE:  reads a frame of audio data from a file to the buffer,
*   aligns the data for future processing, and separates the
*   left and right channels
*
*
************************************************************************/
unsigned long
get_audio (FILE * musicin,
	   short FAR buffer[2][1152],
	   unsigned long num_samples, int stereo, layer * info)
{
  int j;
  short insamp[2304];
  unsigned long samples_read;

  if (stereo == 2)
    {				/* stereo */
      samples_read = read_samples (musicin, insamp, num_samples,
				   (unsigned long) 2304);
      if (glopts.channelswap == TRUE)
	{
	  for (j = 0; j < 1152; j++)
	    {
	      buffer[1][j] = insamp[2 * j];
	      buffer[0][j] = insamp[2 * j + 1];
	    }
	}
      else
	{
	  for (j = 0; j < 1152; j++)
	    {
	      buffer[0][j] = insamp[2 * j];
	      buffer[1][j] = insamp[2 * j + 1];
	    }
	}
    }
  else if (glopts.downmix == TRUE)
    {
      samples_read = read_samples (musicin, insamp, num_samples,
				   (unsigned long) 2304);
      for (j = 0; j < 1152; j++)
	{
	  buffer[0][j] = 0.5 * (insamp[2 * j] + insamp[2 * j + 1]);
	}
    }
  else
    {				/* mono */
      samples_read = read_samples (musicin, insamp, num_samples,
				   (unsigned long) 1152);

//	  for (j = 1152; j-- > samples_read; )
//		 insamp[j] = 0;

      for (j = 0; j < 1152; j++)
	{
	  buffer[0][j] = insamp[j];
	  /* buffer[1][j] = 0;  don't bother zeroing this buffer. MFC Nov 99 */
	}
    }
  return (samples_read);
}


/*****************************************************************************
*
*  Routines to determine byte order and swap bytes
*
*****************************************************************************/

enum byte_order
DetermineByteOrder (void)
{
  char s[sizeof (long) + 1];
  union
  {
    long longval;
    char charval[sizeof (long)];
  }
  probe;
  probe.longval = 0x41424344L;	/* ABCD in ASCII */
  strncpy (s, probe.charval, sizeof (long));
  s[sizeof (long)] = '\0';
  /* fprintf( stderr, "byte order is %s\n", s ); */
  if (strcmp (s, "ABCD") == 0)
    return order_bigEndian;
  else if (strcmp (s, "DCBA") == 0)
    return order_littleEndian;
  else
    return order_unknown;
}

void
SwapBytesInWords (short *loc, int words)
{
  int i;
  short thisval;
  char *dst, *src;
  src = (char *) &thisval;
  for (i = 0; i < words; i++)
    {
      thisval = *loc;
      dst = (char *) loc++;
      dst[0] = src[1];
      dst[1] = src[0];
    }
}

/*****************************************************************************
 *
 *  Read Audio Interchange File Format (AIFF) headers.
 *
 *****************************************************************************/

int
aiff_read_headers (FILE * file_ptr, IFF_AIFF * aiff_ptr)
{
  int chunkSize, subSize, sound_position;

  if (fseek (file_ptr, 0, SEEK_SET) != 0)
    return -1;

  if (Read32BitsHighLow (file_ptr) != IFF_ID_FORM)
    return -1;

  chunkSize = Read32BitsHighLow (file_ptr);

  if (Read32BitsHighLow (file_ptr) != IFF_ID_AIFF)
    return -1;

  sound_position = 0;
  while (chunkSize > 0)
    {
      chunkSize -= 4;
      switch (Read32BitsHighLow (file_ptr))
	{

	case IFF_ID_COMM:
	  chunkSize -= subSize = Read32BitsHighLow (file_ptr);
	  aiff_ptr->numChannels = Read16BitsHighLow (file_ptr);
	  subSize -= 2;
	  aiff_ptr->numSampleFrames = Read32BitsHighLow (file_ptr);
	  subSize -= 4;
	  aiff_ptr->sampleSize = Read16BitsHighLow (file_ptr);
	  subSize -= 2;
	  aiff_ptr->sampleRate = ReadIeeeExtendedHighLow (file_ptr);
	  subSize -= 10;
	  while (subSize > 0)
	    {
	      getc (file_ptr);
	      subSize -= 1;
	    }
	  break;

	case IFF_ID_SSND:
	  chunkSize -= subSize = Read32BitsHighLow (file_ptr);
	  aiff_ptr->blkAlgn.offset = Read32BitsHighLow (file_ptr);
	  subSize -= 4;
	  aiff_ptr->blkAlgn.blockSize = Read32BitsHighLow (file_ptr);
	  subSize -= 4;
	  sound_position = ftell (file_ptr) + aiff_ptr->blkAlgn.offset;
	  if (fseek (file_ptr, (long) subSize, SEEK_CUR) != 0)
	    return -1;
	  aiff_ptr->sampleType = IFF_ID_SSND;
	  break;

	default:
	  chunkSize -= subSize = Read32BitsHighLow (file_ptr);
	  while (subSize > 0)
	    {
	      getc (file_ptr);
	      subSize -= 1;
	    }
	  break;
	}
    }
  return sound_position;
}

/*****************************************************************************
 *
 *  Seek past some Audio Interchange File Format (AIFF) headers to sound data.
 *
 *****************************************************************************/

int
aiff_seek_to_sound_data (FILE * file_ptr)
{
  if (fseek
      (file_ptr, AIFF_FORM_HEADER_SIZE + AIFF_SSND_HEADER_SIZE,
       SEEK_SET) != 0)
    return (-1);
  return (0);
}
