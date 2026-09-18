#include "common.h"

/*****************************************************************************
*
*  CRC error protection package
*
*****************************************************************************/

void
CRC_calc (frame_params * fr_ps,
	  unsigned int bit_alloc[2][SBLIMIT],
	  unsigned int scfsi[2][SBLIMIT], unsigned int *crc)
{
  int i, k;
  layer *info = fr_ps->header;
  int stereo = fr_ps->stereo;
  int sblimit = fr_ps->sblimit;
  int jsbound = fr_ps->jsbound;
  al_table *alloc = fr_ps->alloc;

  *crc = 0xffff;		/* changed from '0' 92-08-11 shn */
  update_CRC (info->bitrate_index, 4, crc);
  update_CRC (info->sampling_frequency, 2, crc);
  update_CRC (info->padding, 1, crc);
  update_CRC (info->extension, 1, crc);
  update_CRC (info->mode, 2, crc);
  update_CRC (info->mode_ext, 2, crc);
  update_CRC (info->copyright, 1, crc);
  update_CRC (info->original, 1, crc);
  update_CRC (info->emphasis, 2, crc);

  for (i = 0; i < sblimit; i++)
    for (k = 0; k < ((i < jsbound) ? stereo : 1); k++)
      update_CRC (bit_alloc[k][i], (*alloc)[i][0].bits, crc);

  for (i = 0; i < sblimit; i++)
    for (k = 0; k < stereo; k++)
      if (bit_alloc[k][i])
	update_CRC (scfsi[k][i], 2, crc);
}

void
update_CRC (unsigned int data, unsigned int length, unsigned int *crc)
{
  unsigned int masking, carry;

  masking = 1 << length;

  while ((masking >>= 1))
    {
      carry = *crc & 0x8000;
      *crc <<= 1;
      if (!carry ^ !(data & masking))
	*crc ^= CRC16_POLYNOMIAL;
    }
  *crc &= 0xffff;
}

void
CRC_calcDAB (frame_params * fr_ps,
	     unsigned int bit_alloc[2][SBLIMIT],
	     unsigned int scfsi[2][SBLIMIT],
	     unsigned int scalar[2][3][SBLIMIT],
	     unsigned int *crc, int packed)
{
  int i, j, k;
  int stereo = fr_ps->stereo;
  int nb_scalar;
  int f[5] = { 0, 4, 8, 16, 30 };
  int first, last;

  first = f[packed];
  last = f[packed + 1];
  if (last > fr_ps->sblimit)
    last = fr_ps->sblimit;

  nb_scalar = 0;
  *crc = 0x0;
  for (i = first; i < last; i++)
    for (k = 0; k < stereo; k++)
      if (bit_alloc[k][i])	/* above jsbound, bit_alloc[0][i] == ba[1][i] */
	switch (scfsi[k][i])
	  {
	  case 0:
	    for (j = 0; j < 3; j++)
	      {
		nb_scalar++;
		update_CRCDAB (scalar[k][j][i] >> 3, 3, crc);
	      }
	    break;
	  case 1:
	  case 3:
	    nb_scalar += 2;
	    update_CRCDAB (scalar[k][0][i] >> 3, 3, crc);
	    update_CRCDAB (scalar[k][2][i] >> 3, 3, crc);
	    break;
	  case 2:
	    nb_scalar++;
	    update_CRCDAB (scalar[k][0][i] >> 3, 3, crc);
	  }
}

void
update_CRCDAB (unsigned int data, unsigned int length, unsigned int *crc)
{
  unsigned int masking, carry;

  masking = 1 << length;

  while ((masking >>= 1))
    {
      carry = *crc & 0x80;
      *crc <<= 1;
      if (!carry ^ !(data & masking))
	*crc ^= CRC8_POLYNOMIAL;
    }
  *crc &= 0xff;
}
