//===============================================================================
//  #35 Bowman Wright : developer of the Golden Snitch 
//===============================================================================

class  WCWright extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardWrightTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wrightsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWrightBigTexture FILE=TEXTURES\menu\Folio\Cards\Wrightbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Bowman Wright";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=35
    skin(0)=Texture'HProps.skins.WizardCardWrightTex0'
	textureBig=Texture'WizCardWrightBigTexture'
	strDescriptionId="WizCard_0031"
}
