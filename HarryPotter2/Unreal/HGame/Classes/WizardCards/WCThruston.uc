//===============================================================================
// #78 Orsino Thruston
//===============================================================================

class  WCThruston extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardThrustonTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Thrustonsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardThrustonBigTexture FILE=TEXTURES\menu\Folio\Cards\Thrustonbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Orsino Thruston";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=78
    skin(0)=Texture'HProps.skins.WizardCardThrustonTex0'
	textureBig=Texture'WizCardThrustonBigTexture'
	strDescriptionId="WizCard_0072"
}
