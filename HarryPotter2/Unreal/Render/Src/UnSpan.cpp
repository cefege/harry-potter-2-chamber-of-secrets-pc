/*=============================================================================
    UnSpan.cpp: Unreal span buffering functions
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.
=============================================================================*/

#include "Render.h"
#include "hp2_rs.h"

/*-----------------------------------------------------------------------------
    Span buffer algorithms: forwarders to crates/hp2-render/src/span.rs.
    The FSpanBuffer layout and the Assert* debugging functions stay here.
-----------------------------------------------------------------------------*/

void FSpanBuffer::AllocIndex( int AllocStartY, int AllocEndY, FMemStack* MemStack )
{
    guard(FSpanBuffer::AllocIndex);
    hp2_rs_render_span_alloc_index(GRenderServices, this, AllocStartY, AllocEndY, MemStack);
    unguardf((TEXT("(%i-%i)"), AllocStartY, AllocEndY));
}

void FSpanBuffer::AllocIndexForScreen( INT SXR, INT SYR, FMemStack* MemStack )
{
    guard(FSpanBuffer::AllocIndexForScreen);
    hp2_rs_render_span_alloc_index_for_screen(GRenderServices, this, SXR, SYR, MemStack);
    unguard;
}

void FSpanBuffer::Release()
{
    guard(FSpanBuffer::Release);
    hp2_rs_render_span_release(GRenderServices, this);
    unguard;
}

void FSpanBuffer::GetValidRange( SWORD* ValidStartY, SWORD* ValidEndY )
{
    hp2_rs_render_span_get_valid_range(this, ValidStartY, ValidEndY);
}

INT FSpanBuffer::BoxIsVisible( INT X1, INT Y1, INT X2, INT Y2 )
{
    INT Result;
    guard(FSpanBuffer::BoxIsVisible);
    Result = hp2_rs_render_span_box_is_visible(this, X1, Y1, X2, Y2);
    unguard;
    return Result;
}

INT FSpanBuffer::CopyFromRasterUpdate( FSpanBuffer& Screen, INT RasterStartY, INT RasterEndY, FRasterSpan* Raster )
{
    INT Result;
    guard(FSpanBuffer::CopyFromRasterUpdate);
    Result = hp2_rs_render_span_copy_from_raster_update(GRenderServices, this, &Screen, RasterStartY, RasterEndY, Raster);
    unguard;
    return Result;
}

INT FSpanBuffer::CopyFromRaster( FSpanBuffer& Screen, INT RasterStartY, INT RasterEndY, FRasterSpan* Raster )
{
    INT Result;
    guard(FSpanBuffer::CopyFromRaster);
    Result = hp2_rs_render_span_copy_from_raster(GRenderServices, this, &Screen, RasterStartY, RasterEndY, Raster);
    unguard;
    return Result;
}

void FSpanBuffer::MergeWith( const FSpanBuffer& Other )
{
    guard(FSpanBuffer::MergeWith);
    hp2_rs_render_span_merge_with(GRenderServices, this, const_cast<FSpanBuffer*>(&Other));
    unguard;
}

void FSpanBuffer::CopyIndexFrom( const FSpanBuffer& Source, FMemStack* Mem )
{
    guard(FSpanBuffer::CopyIndexFrom);
    hp2_rs_render_span_copy_index_from(GRenderServices, this, const_cast<FSpanBuffer*>(&Source), Mem);
    unguard;
}

/*-----------------------------------------------------------------------------
    Debugging.
-----------------------------------------------------------------------------*/

//
// These debugging functions are available while writing span buffer code.
// They perform various checks to make sure that span buffers don't become
// corrupted.  They don't need optimizing, of course.
//

//
// Make sure that a span buffer is completely empty.
//
void FSpanBuffer::AssertEmpty( TCHAR* Name )
{
    guard(FSpanBuffer::AssertEmpty);
    FSpan **TempIndex,*Span;
    int i;

    TempIndex = Index;
    for( i=StartY; i<EndY; i++ )
    {
        Span = *(TempIndex++);
        while (Span!=NULL)
        {
            appErrorf( TEXT("%s not empty, line=%i<%i>%i, start=%i, end=%i"), Name, StartY, i, EndY, Span->Start, Span->End );
            Span=Span->Next;
        }
    }
    unguard;
}

//
// Assure that a span buffer isn't empty.
//
void FSpanBuffer::AssertNotEmpty( TCHAR* Name )
{
    guard(FSpanBuffer::AssertNotEmpty);
    FSpan **TempIndex,*Span;
    int i,NotEmpty=0;

    TempIndex = Index;
    for( i=StartY; i<EndY; i++ )
    {
        Span = *(TempIndex++);
        while (Span!=NULL)
        {
            if( Span->Start>=Span->End )
				appErrorf( TEXT("%s contains %i-length span"), Name, Span->End-Span->Start );
            NotEmpty=1;
            Span=Span->Next;
        }
    }
    if( !NotEmpty )
		appErrorf( TEXT("%s is empty"), Name );
    unguard;
}

//
// Make sure that a span buffer is valid.  Performs the following checks:
// - Make sure there are no zero-length spans
// - Make sure there are no negative-length spans
// - Make sure there are no overlapping spans
// - Make sure all span pointers are valid (otherwise GPF's)
//
void FSpanBuffer::AssertValid( TCHAR* Name )
{
    guard(FSpanBuffer::AssertValid);

    FSpan **TempIndex,*Span;
    int i,PrevEnd,c=0;

    TempIndex = Index;
    for( i=StartY; i<EndY; i++ )
    {
        PrevEnd = -1000;
        Span = *(TempIndex++);
        while( Span )
        {
            if ((i==StartY)||(i==(EndY-1)))
            {
                if ((PrevEnd!=-1000) && (PrevEnd >= Span->Start)) appErrorf(TEXT("%s contains %i-length overlap, line %i/%i"),Name,PrevEnd-Span->Start,i-StartY,EndY-StartY);
                if (Span->Start>=Span->End) appErrorf(TEXT("%s contains %i-length span, line %i/%i"),Name,Span->End-Span->Start,i-StartY,EndY-StartY);
                PrevEnd = Span->End;
            }
            Span=Span->Next;
            c++;
        }
    }
    if( c!=ValidLines )
		appErrorf( TEXT("%s bad ValidLines: claimed=%i, correct=%i"), Name, ValidLines, c );
    unguard;
}

//
// Like AssertValid, but 'ValidLines' checked for zero/nonzero only.
//
void FSpanBuffer::AssertGoodEnough( TCHAR* Name )
{
    guard(FSpanBuffer::AssertGoodEnough);
    FSpan **TempIndex,*Span;
    int i,PrevEnd,c=0;

    TempIndex = Index;
    for( i=StartY; i<EndY; i++ )
    {
        PrevEnd = -1000;
        Span = *(TempIndex++);
        while (Span)
        {
            if( (i==StartY)||(i==(EndY-1)) )
            {
                if ((PrevEnd!=-1000) && (PrevEnd >= Span->Start)) appErrorf(TEXT("%s contains %i-length overlap, line %i/%i"),Name,PrevEnd-Span->Start,i-StartY,EndY-StartY);
                if (Span->Start>=Span->End) appErrorf(TEXT("%s contains %i-length span, line %i/%i"),Name,Span->End-Span->Start,i-StartY,EndY-StartY);
                PrevEnd = Span->End;
            }
            Span=Span->Next;
            c++;
        }
    }
    if( (c==0) != (ValidLines==0) )
		appErrorf( TEXT("%s bad ValidLines: claimed=%i, correct=%i"), Name, ValidLines, c );
    unguard;
}

/*-----------------------------------------------------------------------------
    The End.
-----------------------------------------------------------------------------*/
