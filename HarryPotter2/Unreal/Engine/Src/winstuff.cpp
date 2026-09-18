#include <windows.h>

__int64 GetTimeIndex ()
{
	LARGE_INTEGER query;
	BOOL bOkay = QueryPerformanceCounter (&query);

	if (!bOkay)
		return -1;

	return query.QuadPart;
}
