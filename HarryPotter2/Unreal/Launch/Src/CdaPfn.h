/***************************************************************************************************
 * Name: CdaPfn.h
 *
 * $Revision: 8 $
 *
 * Description:	Public header for Application Security Functions
 *
 *
 *	This header contains the macro declarations needed to declare a function as
 *  a SafeDisc API Protected Function.
 *
 *	For full details see the SafeDisc API User Guide
 *
 ***************************************************************************************************
 * NOTIFICATION OF COPYRIGHT AND OWNERSHIP OF SOFTWARE:
 *
 * Copyright (c) 2000, C-Dilla Ltd.  All Rights Reserved.
 *
 * This computer program is the property of C-Dilla Ltd. of Reading,
 * Berkshire, England, and Macrovision Corp. of Sunnyvale, California, U.S.A.
 * Authorization granted to a party for its use is limited to the terms of a
 * written agreement signed by C-Dilla Ltd. or Macrovision Corp.  Any
 * derivatives and/or enhancements of this computer program related to such
 * use remain the property of C-Dilla Ltd. of Reading, Berkshire, England, and
 * Macrovision Corp. of Sunnyvale, California, U.S.A.
 *
 ***************************************************************************************************
 * 
 * $Nokeywords: $
 *
 **************************************************************************************************/

#ifndef _CDAPFN_H
#define _CDAPFN_H

// Version the macros

#define	CDAPFN_VERSION		4

// Declare a structure that will be instanced and exported for each ASF.
// The structure packing pragma is needed to make sure that SafeDisc has the same
// view of the structure as the application.

#pragma pack(push, 4)

typedef void (*CDAPFN_PTR)();

typedef struct
{
	unsigned long Magic;				// Identifies this as a protected function property block
										// (used for integrity checks)
	unsigned long Version;				// Identifies this structure's version by instance
										// (allowing objects using different versions to be linked)
	unsigned long Size;					// Size of this structure in bytes
	unsigned long FunctionAddress;		// The address of the protected function
	unsigned long MaxOverhead;			// Maximum execution overhead
	unsigned long Constraints;			// Protection constraints
	char 		  *PrivateStr;
	unsigned long State;				// Initial state is 0
	unsigned long ProtectionGroup;		// The group that this function belongs to
	unsigned long AdtControl;			// Type of anti-debugger protection
	unsigned long EncryptionType;		// Type of encryption
	unsigned long SequenceId;			// Sequence identifier
	unsigned char Private[84];

} CDAPFN_PROPERTIES, *PCDAPFN_PROPERTIES, **PPCDAPFN_PROPERTIES;

#define	CDAPFN_MAGIC	0xD4F70933	

#pragma pack(pop)

//******************************************************************************
// Protection properties
//******************************************************************************

// Execution overhead levels

// Comments provide guidelines for the increase in the execution time of the function.
// The guidelines are based on a P500 processor and a function length of 500 bytes.
// At version 1 
//		encryption is provided at all levels
//		anti-debugger checks are performed - most extensive at L5. least at L1

#define CDAPFN_OVERHEAD_L5		5	// Guideline: 10 seconds
#define CDAPFN_OVERHEAD_L4		4	// Guideline: 1 second
#define CDAPFN_OVERHEAD_L3		3	// Guideline: 500 milliseconds
#define CDAPFN_OVERHEAD_L2		2	// Guideline: 100 milliseconds
#define CDAPFN_OVERHEAD_L1		1	// Guideline: 50 milliseconds

#define CDAPFN_OVERHEAD_ENCODING_ENCRYPT 0
#define CDAPFN_OVERHEAD_ENCODING_MANGLE  1 

#define CDAPFN_OVERHEAD_LEVEL( Level )               ( Level % 6 )  
#define CDAPFN_OVERHEAD_ENCODING_TYPE( EncodeType )  ( ( EncodeType % 2 ) * 0x10000 )
#define CDAPFN_OVERHEAD_ENCODING_DENSITY( Percent )  ( ( Percent % 100 ) * 0x100 ) 

// Constraints

	// No constraints

#define CDAPFN_CONSTRAINT_NONE					0x00000000

	// Function contains an exception handler (called functions may return to it without
	// normal stack unwinding)

#define CDAPFN_CONSTRAINT_EXCEPTION_HANDLER		0x00000001
#define CDAPFN_CONSTRAINT_EXLUDE_FROM_RUNTIME	0x00000002

// Defaulted properties.  Do not change these values

#define	CDAPFN_PROTECTION_GROUP_DEFAULT		1
#define	CDAPFN_STATE_INITIAL_USER			0

//******************************************************************************
// Macros needed to declare a global function as an API Protected Function
//******************************************************************************

/*
	Steps needed
	1.	Prototype the function (if not done elsewhere, e.g. in a header)
	2	Invoke CDAPFN_DECLARE_GLOBAL before the function declaration
	3.	Invoke CDAPFN_ENDMARK exactly once inside the function
	
	Function code before CDAPFN_ENDMARK is protected, code after is not.
	Care must be taken not to position this macro where the compiler might optimise it out,
	for example after the return statement.

	Example:
		
	int MyFunc();					// Must prototype the function if not done elsewhere

	CDAPFN_DECLARE_GLOBAL(MyFunc, CDAPFN_OVERHEAD_L5, CDAPFN_CONSTRAINT_NONE);

	int MyFunc()
	{
		// My protected stuff

		CDAPFN_ENDMARK(MyFunc);	// Marks end of protected function
		return 0;				
	}
*/

#define CDAPFN_DECLARE_PROPERTIES(FnName, MaxOverhead, Constraints, SequenceId ) \
	__declspec(dllexport) CDAPFN_PROPERTIES CDAPFN0506_##FnName =				\
	{																			\
		CDAPFN_MAGIC,															\
		CDAPFN_VERSION,															\
		sizeof(CDAPFN_PROPERTIES),												\
		(unsigned long)FnName,													\
		(MaxOverhead),															\
		(Constraints),															\
		(char *)0,																\
		CDAPFN_STATE_INITIAL_USER,												\
		CDAPFN_PROTECTION_GROUP_DEFAULT,										\
		(unsigned long)0,																		\
		(unsigned long)0,																		\
		(SequenceId)															\
	};

#define CDAPFN_DECLARE_GLOBAL_SEQUENCED( FnName, MaxOverhead, Constraints, SequenceId )	\
\
	CDAPFN_DECLARE_PROPERTIES( FnName, MaxOverhead, Constraints, SequenceId ) \
	void  CDAPFN0506_X_##FnName();\
	CDAPFN_DECLARE_PROPERTIES( CDAPFN0506_X_##FnName, MaxOverhead, \
		Constraints | CDAPFN_CONSTRAINT_EXLUDE_FROM_RUNTIME, 0 ) \
	void  CDAPFN0506_X_##FnName() \
	{\
		if (CDAPFN0506_##FnName.Version == 0)\
		{\
			CDAPFN0506_X_##FnName();\
		}\
		CDAPFN_ENDMARK( CDAPFN0506_X_##FnName );\
		return;\
	}


#define CDAPFN_DECLARE_GLOBAL( FnName, MaxOverhead, Constraints ) \
\
	CDAPFN_DECLARE_GLOBAL_SEQUENCED( FnName, MaxOverhead, Constraints, 0 )


#define CDAPFN_ENDMARK(FnName)													\
	if (CDAPFN0506_##FnName##.Version == 0)	/* always false	*/				\
	{	/* never come here, though compiler can't know that	*/					\
		((CDAPFN_PTR)CDAPFN0506_##FnName##.FunctionAddress)();					\
	}											

#endif	// def _CDAPFN_H
