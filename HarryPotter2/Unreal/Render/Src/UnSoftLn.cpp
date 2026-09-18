/*=============================================================================
	UnSoftLn.cpp: Unreal software line drawing.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "Render.h"

/*-----------------------------------------------------------------------------
	Projection.
-----------------------------------------------------------------------------*/

//
// Figure out the unclipped screen location of a 3D point taking into account either
// a perspective or orthogonal projection.  Returns 1 if view is orthogonal or point 
// is visible in 3D view, 0 if invisible in 3D view (behind the viewer).
//
// Scale = scale of one world unit (at this point) relative to screen pixels,
// for example 0.5 means one world unit is 0.5 pixels.
//
UBOOL URender::Project( FSceneNode* Frame, const FVector& V, FLOAT& ScreenX, FLOAT& ScreenY, FLOAT* Scale )
{
	guard(URender::Project);

	FVector	Temp = V - Frame->Coords.Origin;
	if( Frame->Viewport->Actor->RendMap == REN_OrthXY )
	{
		ScreenX = +Temp.X / Frame->Zoom + Frame->FX2;
		ScreenY = +Temp.Y / Frame->Zoom + Frame->FY2;
		if( Scale )
			*Scale = 1.0f/Frame->Zoom;
		return 1;
	}
	else if( Frame->Viewport->Actor->RendMap==REN_OrthXZ )
	{
		ScreenX = +Temp.X / Frame->Zoom + Frame->FX2;
		ScreenY = -Temp.Z / Frame->Zoom + Frame->FY2;
		if( Scale )
			*Scale = 1.0f/Frame->Zoom;
		return 1;
	}
	else if( Frame->Viewport->Actor->RendMap==REN_OrthYZ )
	{
		ScreenX = +Temp.Y / Frame->Zoom + Frame->FX2;
		ScreenY = -Temp.Z / Frame->Zoom + Frame->FY2;
		if( Scale )
			*Scale = 1.0f/Frame->Zoom;
		return 1;
	}
	else
	{
		Temp     = Temp.TransformVectorBy( Frame->Coords );
		FLOAT Z  = Temp.Z; if (Abs(Z)<0.01f) Z+=0.02f;
		FLOAT RZ = Frame->Proj.Z / Z;
		ScreenX = Temp.X * RZ + Frame->FX2;
		ScreenY = Temp.Y * RZ + Frame->FY2;

		if( Scale  )
			*Scale = RZ;

		return Z > 1.0f;
	}
	unguard;
}

//
// Convert a particular screen location to a world location.  In ortho views,
// sets non-visible component to zero.  In persp views, places at viewport location
// unless UseEdScan=1 and the user just clicked on a wall (a Bsp polygon).
// Sets V to location and returns 1, or returns 0 if couldn't perform conversion.
//
UBOOL URender::Deproject( FSceneNode* Frame, INT ScreenX, INT ScreenY, FVector& V )
{
	guard(URender::Deproject);

	FVector  Origin = Frame->Coords.Origin;
	FLOAT	 SX		= (FLOAT)ScreenX - Frame->FX2;
	FLOAT	 SY		= (FLOAT)ScreenY - Frame->FY2;

	switch( Frame->Viewport->Actor->RendMap )
	{
		case REN_OrthXY:
			V.X = +SX * Frame->Zoom + Origin.X;
			V.Y = +SY * Frame->Zoom + Origin.Y;
			V.Z = 0;
			return 1;
		case REN_OrthXZ:
			V.X = +SX * Frame->Zoom + Origin.X;
			V.Y = 0.0;
			V.Z = -SY * Frame->Zoom + Origin.Z;
			return 1;
		case REN_OrthYZ:
			V.X = 0.0;
			V.Y = +SX * Frame->Zoom + Origin.Y;
			V.Z = -SY * Frame->Zoom + Origin.Z;
			return 1;
		default:
			V = Origin;
			return 0;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Cylinder drawing.
-----------------------------------------------------------------------------*/

typedef double SReal;

int SolveQuadratic(float a_results[2], float a, float b, float c)
{
	if (a == 0.0f)
	{
		//
		// It's a linear equation b x + c = 0, and we must solve it as such.
		// x = -c / b
		//
		
		if (b == 0.0f)
			// No solution.
			return 0;

		a_results[0] = a_results[1] = -c / b;
		return 1;
	}

	//
	// As everyone knows, the solution is:
	//		( -b/2 ± sqrt((b/2)² - a c) ) / a
	//
	b *= -0.5f;
	SReal discrim = b*b - a*c;
	if (discrim < 0.0f)
		// No real solution.
		return 0;

	if (discrim == 0.0f)
	{
		// 1 distinct result.
		a_results[0] = a_results[1] = b / a;
		return 1;
	}

	// 2 distinct results.
	discrim = appSqrt(discrim);
	if (a < 0)
		// Negate, to keep results in ascending order.
		discrim = -discrim;
	SReal inva = 1.0 / a;
	a_results[0] = (b - discrim) * inva;
	a_results[1] = (b + discrim) * inva;
	return 2;
}

template<class T> void Swap( T& a, T& b )
{
	T t = a;
	a = b;
	b = t;
}

//
// Draw a cylinder.
//
void URender::DrawCylinder
(
	FSceneNode*		Frame,
	FPlane			Color,
	DWORD			LineFlags,
	const FCoords&	Place,
	FLOAT			Radius,			// Radius on Y axis if cylinder is oval
	FLOAT			Height,
	FLOAT			Width /*=-1*/	// Radius on X axis if cylinder is to be oval
)
{
	guard(URender::DrawCylinder);

	bool	bOval = ( Width >= 0.0 );
	bool	bSwapped = false;

	// Transform into frame space.
	FCoords FrCoords = Place;

	if (!(LineFlags & LINE_PreTransformed))
	{
		FrCoords *= Frame->Coords;
		LineFlags |= LINE_PreTransformed;
	}

	/*
		Perspective:
			N*(C+rN) = 0
			N*C = -r
			N = Xc + Ys, c² + s² = 1

			(Xc+Ys)*C = -r
			X*C c + r = -Y*C s
			(X*C)² c² + 2(X*C)r c + r² = (Y*C)² (1-c²)
			((X*C)² + (Y*C)²) c² + 2r(X*C) c + r² - (Y*C)² = 0
			c = (-rXC +- sqrt( r² XC² - (XC² + YC²)(r² - YC²) )) / (XC² + YC²)
			  = (-rXC +- YC sqrt( r² - XC² - YC² )) / (XC² + YC²)

			Then s = (-r - XC c)/YC
			N = X c - Y/YC (r + XC c)
			  = (X - Y/YC XC) c - Y/YC r
	*/

	// Draw the 2 bounding lines of the cylinder silhouette.
	FVector Point0 = FrCoords.Origin - FrCoords.ZAxis * Height,
			Point1 = FrCoords.Origin + FrCoords.ZAxis * Height;
	if (Frame->Viewport->IsOrtho())	// Orthographic
	{
		if( FrCoords.ZAxis.SizeSquared2D() > 0.f )
		{
			// Get the vector perpendicular to the camera axis and cylinder axis.
			FVector Offset = FVector(0,0,1) ^ FrCoords.ZAxis;
			Offset = Offset.SafeNormal();
			if ( bOval )			// Oval
			{
				FLOAT	OvalRadialX2 = Square( Offset | FrCoords.XAxis );
				FLOAT	OvalRadius;	// Radius across oval where it's parallel to screen plane
				if ( Width < 0.001 )
					OvalRadius = (OvalRadialX2 < 0.0001) ? 0.0 : Radius;
				else if ( Radius < 0.001 )
					OvalRadius = (1-OvalRadialX2 < 0.0001) ? 0.0 : Width;
				else
				{
					// Solve for length of radial = 1 / sqrt(Ox²/W²+Oy²/R²);
					OvalRadius = OvalRadialX2 / Square(Width) + (1-OvalRadialX2) / Square(Radius);
					OvalRadius = 1.0 / appSqrt( OvalRadius );
				}
				Offset *= OvalRadius;
			}
			else					// Circular
				Offset *= Radius;

			// Draw the 2 bounding lines of the cylinder silhouette.
			Draw3DLine( Frame, Color, LineFlags, Point0 + Offset, Point1 + Offset );
			Draw3DLine( Frame, Color, LineFlags, Point0 - Offset, Point1 - Offset );
		}
	}
	else							// Perspective
	{
		FVector XAxis = FrCoords.XAxis, YAxis = FrCoords.YAxis;
		float XC = XAxis | Point0, YC = YAxis | Point0;
		if( Abs(XC) > Abs(YC) )
		{
			Swap(XAxis, YAxis);
			Swap(XC, YC);
			if ( bOval )
			{
				Swap(Width, Radius);
				bSwapped = true;
			}
		}
		float c0[2], c1[2];
		int results0, results1;
		if ( bOval )			// Oval
		{
			if ( Width < 0.0001 )
			{
				results0 = results1 = 1;
				c0[0] = c1[0] =0.0f;
			}
			else
			{
				FLOAT RWRatio = Radius / Width;
				results0 = SolveQuadratic( c0, XC*XC + Square(RWRatio*YC), XC*2.f, 1 - Square(Radius*YC) );
				XC = XAxis | Point1;  YC = YAxis | Point1;
				results1 = SolveQuadratic( c1, XC*XC + Square(RWRatio*YC), XC*2.f, 1 - Square(Radius*YC) );
			}
			if( results0 == results1 )
			{
				// Time to draw.
				float RYC = -1/YC, XYC = -XC/YC;
				for( int i=0; i<results0; i++ )
				{
					FVector Vec0 = XAxis * c0[i] + YAxis * (RYC + XYC*c0[i]);
					FVector Vec1 = XAxis * c1[i] + YAxis * (RYC + XYC*c1[i]);
					Draw3DLine( Frame, Color, LineFlags, Point0 + Vec0, Point1 + Vec1 );
				}
			}
		}
		else					// Circular
		{
			results0 = SolveQuadratic( c0, XC*XC + YC*YC, XC*Radius*2.f, Square(Radius) - YC*YC );
			XC = XAxis | Point1;  YC = YAxis | Point1;
			results1 = SolveQuadratic( c1, XC*XC + YC*YC, XC*Radius*2.f, Square(Radius) - YC*YC );
			if( results0 == results1 )
			{
				// Time to draw.
				float RYC = -Radius/YC, XYC = -XC/YC;
				for( int i=0; i<results0; i++ )
				{
					FVector Vec0 = XAxis * c0[i] + YAxis * (RYC + XYC*c0[i]);
					FVector Vec1 = XAxis * c1[i] + YAxis * (RYC + XYC*c1[i]);
					Draw3DLine( Frame, Color, LineFlags, Point0 + Vec0*Radius, Point1 + Vec1*Radius );
				}
			}
		}

		if ( bSwapped )
			Swap(Width, Radius);	// Swap back to original
	}

	// Draw the 2 3D circles.
	FVector Origin( 0, Radius, -Height );
	FVector P1, P2;
	FVector ZOffset = FrCoords.ZAxis * (Height*2);
	FLOAT Pix;
	bool bBotVisible, bTopVisible;

	P1 = Origin.TransformPointBy( GMath.UnitCoords * FrCoords );
	if ( bOval )
	{
		Origin.Y = 1.0f;
		Pix = ::Max( Radius, Width );
	}
	else
		Pix = Radius;

	// Figure out how many segments to draw
	// and decide whether top or bottom cap is visible.
	if (Frame->Viewport->IsOrtho())
	{
		Pix /= Frame->Zoom;
		bBotVisible = ZOffset.Z > 0.f;
		bTopVisible = ZOffset.Z < 0.f;
	}
	else
	{
		Pix *= Frame->Proj.Z / FrCoords.Origin.Z;
		bBotVisible = -(P1 | ZOffset) <= 0.f;
		bTopVisible = ((P1+ZOffset) | ZOffset) <= 0.f;
	}

	// Draw 1 segment/4pi pixels.
	INT Segs = Clamp( appRound(Pix*0.5f), 6, 64 );
	for( int i=0; i<=Segs; i++ )
	{
		if ( bOval )
		{
			P2 = Origin.TransformVectorBy( GMath.UnitCoords * FRotator(0, (65536*(i+1))/Segs, 0) );
			P2.X *= Width;
			P2.Y *= Radius;
			P2 = P2.TransformPointBy( GMath.UnitCoords * FrCoords );
		}
		else
			P2 = Origin.TransformPointBy( GMath.UnitCoords * FrCoords * FRotator(0, (65536*(i+1))/Segs, 0) );

		FVector Normal = (P1+P2+ZOffset)*0.5f - FrCoords.Origin;
		if( bBotVisible || (Frame->Viewport->IsOrtho()? Normal.Z : (Normal|P1)) < 0.f )
			Draw3DLine( Frame, Color, LineFlags, P1, P2 );
		if( bTopVisible || (Frame->Viewport->IsOrtho()? Normal.Z : (Normal|(P1+ZOffset))) < 0.f )
			Draw3DLine( Frame, Color, LineFlags, P1+ZOffset, P2+ZOffset );
		P1 = P2;
	}

	unguard;
}

/*-----------------------------------------------------------------------------
	Circle drawing.
-----------------------------------------------------------------------------*/

//
// Draw a circle.
//
void URender::DrawCircle
(
	FSceneNode*		Frame,
	FPlane			Color,
	DWORD			LineFlags,
	FVector&		Location,
	FLOAT			Radius,
	UBOOL			bScaleRadiusByZoom
)
{
	guard(URender::DrawCircle);

	FVector A = Frame->Coords.XAxis;
	FVector B = Frame->Coords.YAxis;

	// Draw 1 segment/4pi pixels.
	FLOAT Pix = Radius;
	if (Frame->Viewport->IsOrtho())
		Pix /= Frame->Zoom;
	else
		Pix *= Frame->Proj.Z / ((Location - Frame->Coords.Origin) | Frame->Coords.ZAxis);
	INT Subdivide = Clamp( appRound(Pix), 6, 64 );

	FLOAT   F  = 0.0;
	FLOAT   AngleDelta = 2.0f*PI / Subdivide;
	FLOAT	ScaledRadius = (bScaleRadiusByZoom ? (Radius * (Frame->Viewport->Actor->OrthoZoom / 10000.0f)) : Radius);
	ScaledRadius = ::Max( ScaledRadius, Radius );	// Never go below the original value

	FVector P1 = Location + ScaledRadius * (A * appCos(F) + B * appSin(F));

	for( int i=0; i<Subdivide; i++ )
	{
		F          += AngleDelta;
		FVector P2  = Location + ScaledRadius * (A * appCos(F) + B * appSin(F));
		Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LineFlags, P1, P2 );
		P1 = P2;
	}
	unguard;
}

/*-----------------------------------------------------------------------------
	Box drawing.
-----------------------------------------------------------------------------*/

//
// Draw a box centered about a location.
//
void URender::DrawBox
(
	FSceneNode*		Frame,
	FPlane			Color,
	DWORD			LineFlags,
	FVector			Min,
	FVector			Max
)
{
	guard(URender::DrawBox);

	FVector A,B;
	FVector Location = Min+Max;
	if	   ( Frame->Viewport->Actor->RendMap==REN_OrthXY )	{A=FVector(Max.X-Min.X,0,0); B=FVector(0,Max.Y-Min.Y,0);}
	else if( Frame->Viewport->Actor->RendMap==REN_OrthXZ )	{A=FVector(Max.X-Min.X,0,0); B=FVector(0,0,Max.Z-Min.Z);}
	else													{A=FVector(0,Max.Y-Min.Y,0); B=FVector(0,0,Max.Z-Min.Z);}

	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LineFlags, (Location+A+B)/2, (Location+A-B)/2 );
	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LineFlags, (Location-A+B)/2, (Location-A-B)/2 );
	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LineFlags, (Location+A+B)/2, (Location-A+B)/2 );
	Frame->Viewport->RenDev->Draw3DLine( Frame, Color, LineFlags, (Location+A-B)/2, (Location-A-B)/2 );

	unguard;
}


inline FVector operator *( const FVector& V, const FCoords& C )
{
	return FVector
	(
		V.X * C.XAxis.X  +  V.Y * C.YAxis.X  +  V.Z * C.ZAxis.X  +  C.Origin.X,
		V.X * C.XAxis.Y  +  V.Y * C.YAxis.Y  +  V.Z * C.ZAxis.Y  +  C.Origin.Y,
		V.X * C.XAxis.Z  +  V.Y * C.YAxis.Z  +  V.Z * C.ZAxis.Z  +  C.Origin.Z
	);
}

//
// Draw a box in arbitrary coords.
//
void URender::DrawBox
(
	FSceneNode*		Frame,
	FPlane			Color,
	DWORD			LineFlags,
	const FBox&		Box,	
	const FCoords&	Coords
)
{
	guard(URender::DrawBox);

	// Transform into frame space.
	FCoords FrCoords = Coords;
	if (!(LineFlags & LINE_PreTransformed))
	{
		FrCoords *= Frame->Coords;
		LineFlags |= LINE_PreTransformed;
	}

	// Draw all 12 lines.
	FVector Min	= Box.Min * FrCoords,
			Max = Box.Max * FrCoords,
			X	= FrCoords.XAxis * (Box.Max.X - Box.Min.X),
			Y	= FrCoords.YAxis * (Box.Max.Y - Box.Min.Y),
			Z	= FrCoords.ZAxis * (Box.Max.Z - Box.Min.Z);

	// Draw only forward faces.
	bool xneg, xpos, yneg, ypos, zneg, zpos;

	if( Frame->Viewport->IsOrtho() )
	{
		xneg = FrCoords.XAxis.Z > 0;		xpos = FrCoords.XAxis.Z < 0;
		yneg = FrCoords.YAxis.Z > 0;		ypos = FrCoords.YAxis.Z < 0;
		zneg = FrCoords.ZAxis.Z > 0;		zpos = FrCoords.ZAxis.Z < 0;
	}
	else
	{
		xneg = (FrCoords.XAxis | Min) > 0;		xpos = (FrCoords.XAxis | Max) < 0;
		yneg = (FrCoords.YAxis | Min) > 0;		ypos = (FrCoords.YAxis | Max) < 0;
		zneg = (FrCoords.ZAxis | Min) > 0;		zpos = (FrCoords.ZAxis | Max) < 0;
	}

	FVector Start;

	// Draw X lines.
	Start = Min;	if (yneg || zneg) Draw3DLine( Frame, Color, LineFlags, Start, Start + X );
	Start += Y;		if (ypos || zneg) Draw3DLine( Frame, Color, LineFlags, Start, Start + X );
	Start += Z;		if (ypos || zpos) Draw3DLine( Frame, Color, LineFlags, Start, Start + X );
	Start -= Y;		if (yneg || zpos) Draw3DLine( Frame, Color, LineFlags, Start, Start + X );

	// Draw Y lines.
	Start = Min;	if (xneg || zneg) Draw3DLine( Frame, Color, LineFlags, Start, Start + Y );
	Start += X;		if (xpos || zneg) Draw3DLine( Frame, Color, LineFlags, Start, Start + Y );
	Start += Z;		if (xpos || zpos) Draw3DLine( Frame, Color, LineFlags, Start, Start + Y );
	Start -= X;		if (xneg || zpos) Draw3DLine( Frame, Color, LineFlags, Start, Start + Y );

	// Draw Z lines.
	Start = Min;	if (xneg || yneg) Draw3DLine( Frame, Color, LineFlags, Start, Start + Z );
	Start += X;		if (xpos || yneg) Draw3DLine( Frame, Color, LineFlags, Start, Start + Z );
	Start += Y;		if (xpos || ypos) Draw3DLine( Frame, Color, LineFlags, Start, Start + Z );
	Start -= X;		if (xneg || ypos) Draw3DLine( Frame, Color, LineFlags, Start, Start + Z );

	unguard;
}


/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
