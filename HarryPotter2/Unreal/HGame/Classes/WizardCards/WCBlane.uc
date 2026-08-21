//===============================================================================
// #31 Balfour Blane
//===============================================================================

class  WCBlane extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardBlaneTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Blanesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBlaneBigTexture FILE=TEXTURES\menu\Folio\Cards\Blanebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Balfour Blane";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=31
    skin(0)=Texture'HProps.skins.WizardCardBlaneTex0'
	textureBig=Texture'WizCardBlaneBigTexture'
	strDescriptionId="WizCard_0016"
}
