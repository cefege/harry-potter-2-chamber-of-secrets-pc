//===============================================================================
//  #92 Xavier Rastrick
//===============================================================================

class  WCRastrick extends silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardRastrickTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Rastricksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardRastrickBigTexture FILE=TEXTURES\menu\Folio\Cards\Rastrickbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Xavier Rastrick";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=92
    skin(0)=Texture'HProps.skins.WizardCardRastrickTex0'
	textureBig=Texture'WizCardRastrickBigTexture'
	strDescriptionId="WizCard_0006"
}
