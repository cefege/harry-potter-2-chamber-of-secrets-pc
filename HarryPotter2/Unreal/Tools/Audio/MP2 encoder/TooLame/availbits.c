/*
 * toolame - an optimized mpeg 1/2 layer 2 audio encoder
 * Copyright (C) 2001 Michael Cheng
 *
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 * 
 *  You should have received a copy of the GNU General Public License
 *  along with this program; if not, write to the Free Software
 *  Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA  02111-1307  USA
 */
#include "common.h"
#include "encoder.h"
#include "musicin.h"
#include "options.h"

int available_bits (layer * info, options * glopts);

struct slotinfo
{
  double average;
  double frac;
  int whole;
  double lag;
  int extra;
}
slots;

/* function returns the number of available bits */
int
available_bits (layer * info, options * glopts)
{
  int adb;

  slots.extra = 0;		/* be default, no extra slots */

  slots.average = (1152.0 / s_freq[info->version][info->sampling_frequency]) *
    ((double) bitrate[info->version][info->bitrate_index] / 8.0);

  slots.whole = (int) slots.average;
  slots.frac = slots.average - (double) slots.whole;
#ifdef VBR
  if (slots.frac != 0 && glopts->usepadbit && glopts->vbr == FALSE)
#else
  if (slots.frac != 0 && glopts->usepadbit)
#endif
    {
      if (slots.lag > (slots.frac - 1.0))	/* no padding for this frame */
	{
	  slots.lag -= slots.frac;
	  slots.extra = 0;
	  info->padding = 0;
	}
      else			/* padding */
	{
	  slots.extra = 1;
	  info->padding = 1;
	  slots.lag += (1 - slots.frac);
	}
    }

  adb = (slots.whole + slots.extra) * 8;

  return adb;
}
