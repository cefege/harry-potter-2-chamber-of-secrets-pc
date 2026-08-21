//===============================================================================
//  #54 Gaspard Shingleton
//===============================================================================

class  WCShingleton extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardShingletonTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Shingletonsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardShingletonBigTexture FILE=TEXTURES\menu\Folio\Cards\Shingletonbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Gaspard Shingleton";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=54
    skin(0)=Texture'HProps.skins.WizardCardShingletonTex0'
	textureBig=Texture'WizCardShingletonBigTexture'
	strDescriptionId="WizCard_0079"
}
