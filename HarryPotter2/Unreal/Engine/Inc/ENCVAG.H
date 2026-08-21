/*
 *
 * EncVag : PlayStation waveform encode routine
 * ver 1.01
 *
 * (C)1998-1999 Sony Computer Entertainment Inc.
 *  All Rights Reserved.
 */

#include <cstdint>

/* function prototypes */
#define  DLLIMP_VOID    extern "C" __declspec( dllimport ) void __stdcall
DLLIMP_VOID EncVagInit( short conversion_mode );
DLLIMP_VOID EncVag( short x[], short y[], short block_attribute );
DLLIMP_VOID EncVagFin( short y[] );

/* conversion_mode */
#define ENC_VAG_MODE_NORMAL	1
#define ENC_VAG_MODE_HIGH	2
#define ENC_VAG_MODE_LOW	3
#define ENC_VAG_MODE_4BIT	4

/* block_attribute */
#define ENC_VAG_1_SHOT		0
#define ENC_VAG_1_SHOT_END	1
#define ENC_VAG_LOOP_START	2
#define ENC_VAG_LOOP_BODY	3
#define ENC_VAG_LOOP_END	4

#define  BLKSIZ  	28		/* block size */

struct VAGheader {
	std::uint32_t		format;		/* always 'VAGp' for identifying*/
	std::uint32_t		ver;		/* format version (2) */
	std::uint32_t		ssa;		/* Source Start Address, always 0 (reserved for VAB format) */
	std::uint32_t		size;		/* Sound Data Size in byte */

	std::uint32_t		fs;			/* sampling frequency, 44100(>pt1000), 32000(>pt), 22000(>pt0800)... */
	std::uint16_t		volL;		/* base volume for Left channel */
	std::uint16_t		volR;		/* base volume for Right channel */
	std::uint16_t		pitch;		/* base pitch (includes fs modulation)*/
	std::uint16_t		ADSR1;		/* base ADSR1 (see SPU manual) */
	std::uint16_t		ADSR2;		/* base ADSR2 (see SPU manual) */
	std::uint16_t		reserved;	/* not in use */

	char				name[16];
};
typedef struct VAGheader VAGheader;
