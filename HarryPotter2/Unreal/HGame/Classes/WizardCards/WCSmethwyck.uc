//===============================================================================
// #70	Leopoldina Smethwyck
//===============================================================================

class  WCSmethwyck extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardSmethwyckTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Smethwycksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardSmethwyckBigTexture FILE=TEXTURES\menu\Folio\Cards\Smethwyckbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Leopoldina Smethwyck";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=70
    skin(0)=Texture'HProps.skins.WizardCardSmethwyckTex0'
	textureBig=Texture'WizCardSmethwyckBigTexture'
	strDescriptionId="WizCard_0064"
}
