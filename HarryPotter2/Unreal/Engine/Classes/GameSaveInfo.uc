class GameSaveInfo extends Object native;

//Sto: all objects in this class are individually saved,
// as I couldn't get the object-level stuff to work.
// If any new items are added here, they will need to be appended
// to the guts of the 'SerializeInfo' function in unscript.cpp

// Shouldn't really be in the engine level at all,
// but only way that works so far

var int numBeans;
var int numStars;
var int numPoints;

// AWRIGHT_111001_001
var int savePointID;

var string currentLevelString;
