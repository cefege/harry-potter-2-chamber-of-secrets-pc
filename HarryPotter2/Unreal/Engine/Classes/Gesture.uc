//=============================================================================
// Gesture: A 2D pattern matchable by a mouse gesture.
//=============================================================================
class Gesture extends Object
	native;

// Array of points. Uses 3D vectors for simplicity.
// To do: Z can hold the time value???
var array<vector> Points;

var array<int> Segments;

native(426) final function float CompareGesture( array <vector> InMousePoints, float fAccuracy );
native(427) final function float CompareGesturePoint( vector InMousePoint, float fAccuracy );

