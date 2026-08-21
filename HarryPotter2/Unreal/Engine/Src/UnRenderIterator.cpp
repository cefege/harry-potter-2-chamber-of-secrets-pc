// UnRenderIterator.cpp

#include "EnginePrivate.h"
#include "UnRender.h"

/*-----------------------------------------------------------------------------
	URenderIterator.
-----------------------------------------------------------------------------*/

URenderIterator::URenderIterator()
:	Index( 0 ),
	Observer( NULL ),
	Frame( NULL )
{
	guard(URenderIterator::URenderIterator);

	check( GetOuter()->IsA( AActor::StaticClass() ) );

	// Check for binary compatibility.
	check(sizeof(URenderIterator)==URenderIterator::StaticClass()->GetPropertiesSize());

	unguard;
}

void URenderIterator::Init( FSceneNode* Camera ) //override to initialize subclass data (call Super required)
{
	guard(URenderIterator::Init);

	Frame = Camera;
	if( Camera ) 
	{
		Observer = Camera->Viewport->Actor;
	}

	unguard;
}

void URenderIterator::UnInit()
{
	guard(URenderIterator::UnInit);

	Frame = NULL;

	unguard;
}

void URenderIterator::First()
{
	guard(URenderIterator::First);

	Index = 0;

	unguard;
}

void URenderIterator::Next()
{
	guard(URenderIterator::Next);

	Index++;

	unguard;
}

bool URenderIterator::IsDone() //override to adjust iteration termination criteria (call Super recommended)
{
	guard(URenderIterator::IsDone);

	return Index >= MaxItems;

	unguard;
}

AActor* URenderIterator::CurrentItem() //override to adjust actor render properties based on Index (call Super recommended)
{
	guard(URenderIterator::CurrentItem);

	return (AActor*)GetOuter();

	unguard;
}

// end of UnRenderIterator.cpp
