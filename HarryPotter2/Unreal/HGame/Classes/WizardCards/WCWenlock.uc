//===============================================================================
//  #32 Bridget Wenlock
//===============================================================================

class  WCWenlock extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWenlockTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Wenlocksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWenlockBigTexture FILE=TEXTURES\menu\Folio\Cards\Wenlockbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Bridget Wenlock";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=32
    skin(0)=Texture'HProps.skins.WizardCardWenlockTex0'
	textureBig=Texture'WizCardWenlockBigTexture'
	strDescriptionId="WizCard_0028"
}
