#ifndef OPTIONS_H
#define OPTIONS_H

typedef struct
{
  int usepsy;			/* TRUE   by default, use the psy model */
  int usepadbit;		/* TRUE   by default, use a padding bit */
  int quickmode;		/* FALSE  calculate psy model for every frame */
  int quickcount;		/* 10     when quickmode = TRUE, calculate psymodel every 10th frame */
  int downmix;			/* FALSE  downmix from stereo to mono */
  int byteswap;			/* FALSE  swap the bytes */
  int channelswap;		/* FALSE  swap the channels */
  int dab;			/* FALSE  DAB extensions */
#ifdef VBR
  int vbr;			/* FALSE  switch for VBR mode */
  int vbrlevel;			/* 0      level of VBR . higher is better */
#endif
}
options;

options glopts;
#endif
