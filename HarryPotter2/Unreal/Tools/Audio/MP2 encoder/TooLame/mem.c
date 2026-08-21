#include <stdlib.h>
#include "common.h"

/*******************************************************************************
*
*  Allocate number of bytes of memory equal to "block".
*
*******************************************************************************/

void FAR *
mem_alloc (unsigned long block, char *item)
{

  void *ptr;

  ptr = (void FAR *) malloc (block);

  if (ptr != NULL)
    {
      memset (ptr, 0, block);
    }
  else
    {
      fprintf (stderr, "Unable to allocate %s\n", item);
      exit (0);
    }
  return (ptr);
}


/****************************************************************************
*
*  Free memory pointed to by "*ptr_addr".
*
*****************************************************************************/

void
mem_free (void **ptr_addr)
{

  if (*ptr_addr != NULL)
    {
      free (*ptr_addr);
      *ptr_addr = NULL;
    }

}
